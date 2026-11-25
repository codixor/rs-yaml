//! Account for nodes before passing them to the compatibility loader, which clones anchors.
use crate::{
    Limits, ParseError, ParseResult, error::Location, parser::is_merge_scalar, source::SourceGuard,
};
use saphyr_parser::{BufferedInput, Event, Parser, Span, Tag};
use std::collections::HashMap;

const MAX_INPUT_BYTES: usize = 8 * 1024 * 1024;
pub(crate) const MAX_DEPTH: usize = 64;
const MAX_DOCUMENTS: usize = 4096;
const MAX_COST: usize = 32 * 1024 * 1024;
const NODE_COST: usize = 128;

/// Compatibility profile. Whole-stream and materialization safety ceilings apply independently.
pub const YAML_LIMITS: Limits = Limits {
    max_depth: MAX_DEPTH as u32,
    max_document_bytes: MAX_INPUT_BYTES as u64,
    max_token_bytes: MAX_INPUT_BYTES,
    max_expansion: MAX_COST as u64,
};

#[derive(Clone, Copy)]
struct Node {
    cost: usize,
    height: usize,
    containers: usize,
    merge_key: bool,
}

impl Node {
    fn new(text_len: usize, tag: Option<&Tag>, collection: bool) -> Self {
        let tag_bytes = tag.map_or(0, |t| t.handle.len() + t.suffix.len());
        Self {
            cost: NODE_COST + text_len + tag_bytes,
            height: 1,
            containers: usize::from(collection),
            merge_key: false,
        }
    }
}

struct Frame {
    anchor: usize,
    node: Node,
    mapping: bool,
    expecting_key: bool,
    merge_value: bool,
}

#[derive(Default)]
struct Budget {
    parents: Vec<Frame>,
    anchors: HashMap<usize, Node>,
    cost: usize,
    expansion: u64,
    documents: usize,
}

impl Budget {
    fn charge(&mut self, cost: usize) -> ParseResult<()> {
        self.cost += cost;
        if self.cost > MAX_COST {
            return Err(ParseError::limit("expanded node and scalar budget"));
        }
        Ok(())
    }

    fn expand(&mut self, cost: usize, limits: &Limits) -> ParseResult<()> {
        self.expansion += cost as u64;
        if self.expansion > limits.max_expansion {
            return Err(ParseError::limit("expansion budget"));
        }
        Ok(())
    }

    fn finish(&mut self, id: usize, node: Node, limits: &Limits) -> ParseResult<()> {
        if self.parents.len() + node.height > MAX_DEPTH
            || self.parents.len() + node.containers > limits.max_depth as usize
        {
            return Err(ParseError::limit("expanded nesting depth"));
        }
        if id != 0 {
            self.charge(node.cost)?;
            self.anchors.insert(id, node);
        }
        if let Some(parent) = self.parents.last_mut() {
            parent.node.cost += node.cost;
            parent.node.height = parent.node.height.max(1 + node.height);
            parent.node.containers = parent.node.containers.max(1 + node.containers);
            if parent.mapping {
                let merge_value = parent.merge_value;
                parent.merge_value = parent.expecting_key && node.merge_key;
                parent.expecting_key = !parent.expecting_key;
                if merge_value {
                    self.expand(node.cost, limits)?;
                }
            }
        }
        Ok(())
    }

    fn observe(&mut self, event: &Event<'_>, limits: &Limits) -> ParseResult<()> {
        match event {
            Event::DocumentStart(_) => {
                self.documents += 1;
                if self.documents > MAX_DOCUMENTS {
                    return Err(ParseError::limit("document count"));
                }
                self.expansion = 0;
                self.anchors.clear();
            }
            Event::MappingStart(id, tag) | Event::SequenceStart(id, tag) => {
                if self.parents.len() >= MAX_DEPTH
                    || self.parents.len() >= limits.max_depth as usize
                {
                    return Err(ParseError::limit("collection nesting depth"));
                }
                let node = Node::new(0, tag.as_deref(), true);
                self.charge(node.cost)?;
                self.parents.push(Frame {
                    anchor: *id,
                    node,
                    mapping: matches!(event, Event::MappingStart(..)),
                    expecting_key: true,
                    merge_value: false,
                });
            }
            Event::MappingEnd | Event::SequenceEnd => {
                if let Some(frame) = self.parents.pop() {
                    self.finish(frame.anchor, frame.node, limits)?;
                }
            }
            Event::Scalar(text, style, id, tag) => {
                let mut node = Node::new(text.len(), tag.as_deref(), false);
                node.merge_key = is_merge_scalar(text, *style, tag.as_deref());
                self.charge(node.cost)?;
                self.finish(*id, node, limits)?;
            }
            Event::Alias(id) => {
                let node = self
                    .anchors
                    .get(id)
                    .copied()
                    .ok_or(ParseError::UnresolvedAlias)?;
                self.charge(node.cost)?;
                self.expand(node.cost, limits)?;
                self.finish(0, node, limits)?;
            }
            Event::Nothing | Event::StreamStart | Event::StreamEnd | Event::DocumentEnd => {}
        }
        Ok(())
    }
}

#[derive(Clone, Copy, PartialEq)]
pub(crate) enum Documents {
    Single,
    Stream,
}

pub(crate) fn validate(input: &str, documents: Documents, limits: &Limits) -> ParseResult<()> {
    load(input, documents, limits, |_, _| Ok(()))
}

pub(crate) fn load<'a>(
    input: &'a str,
    documents: Documents,
    limits: &Limits,
    mut receive: impl FnMut(Event<'a>, Span) -> ParseResult<()>,
) -> ParseResult<()> {
    let cap = if documents == Documents::Single {
        limits.max_document_bytes.min(MAX_INPUT_BYTES as u64) as usize
    } else {
        MAX_INPUT_BYTES
    };
    if input.len() > cap {
        return Err(ParseError::ResourceLimit {
            limit: "input bytes",
            at: Some(Location::at(input, cap)),
        });
    }
    // The scanner uses NUL as its end sentinel. Reject embedded NUL before it can hide a suffix.
    if let Some(i) = input.find('\0') {
        return Err(ParseError::Syntax {
            at: Location::at(input, i),
            category: crate::Category::Syntax,
            message: "NUL in YAML input",
        });
    }
    let mut source = SourceGuard::new(input, limits);
    let mut budget = Budget::default();
    let mut keys = crate::keys::Keys::default();
    for result in Parser::new(BufferedInput::new(input.chars())) {
        let (event, span) = result.map_err(|e| ParseError::from_input(&e, input))?;
        if documents == Documents::Single
            && budget.documents == 1
            && matches!(event, Event::DocumentStart(_))
        {
            return Err(ParseError::MultipleDocuments {
                at: Location::marker(input, span.start),
            });
        }
        if let Some(source) = &mut source {
            source.observe(&event, span)?;
        }
        budget
            .observe(&event, limits)
            .map_err(|error| match error {
                ParseError::ResourceLimit { limit, .. } => ParseError::ResourceLimit {
                    limit,
                    at: Some(Location::marker(input, span.start)),
                },
                other => other,
            })?;
        keys.observe(&event, span.start, input)?;
        receive(event, span)?;
    }
    Ok(())
}
