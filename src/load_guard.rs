//! Account for nodes before handing them to the compatibility loader, which clones anchors.
use crate::{ParseError, ParseResult};
use saphyr_parser::{BufferedInput, Event, Parser, Span, Tag};
use std::collections::HashMap;

pub(crate) const MAX_INPUT_BYTES: usize = 8 * 1024 * 1024;
pub(crate) const MAX_DEPTH: usize = 64;
const MAX_DOCUMENTS: usize = 4096;
// Logical cost includes per-node space and scalar/tag payloads. This is not a process RSS cap.
const MAX_COST: usize = 32 * 1024 * 1024;
const NODE_COST: usize = 128;

#[derive(Clone, Copy)]
struct Node {
    cost: usize,
    height: usize,
}

impl Node {
    fn new(text_len: usize, tag: Option<&Tag>) -> Self {
        let tag_bytes = tag.map_or(0, |t| t.handle.len() + t.suffix.len());
        Self {
            cost: NODE_COST + text_len + tag_bytes,
            height: 1,
        }
    }
}

#[derive(Default)]
struct Budget {
    parents: Vec<(usize, Node)>,
    anchors: HashMap<usize, Node>,
    cost: usize,
    documents: usize,
}

impl Budget {
    fn charge(&mut self, cost: usize) -> ParseResult<()> {
        self.cost = self.cost.saturating_add(cost);
        if self.cost > MAX_COST {
            return Err(ParseError::ResourceLimit("expanded node and scalar budget"));
        }
        Ok(())
    }

    fn finish(&mut self, id: usize, node: Node) -> ParseResult<()> {
        if self.parents.len() + node.height > MAX_DEPTH {
            return Err(ParseError::ResourceLimit("expanded nesting depth"));
        }
        if id != 0 {
            self.charge(node.cost)?;
            self.anchors.insert(id, node);
        }
        if let Some((_, parent)) = self.parents.last_mut() {
            parent.cost += node.cost;
            parent.height = parent.height.max(1 + node.height);
        }
        Ok(())
    }

    fn observe(&mut self, event: &Event<'_>) -> ParseResult<()> {
        match event {
            Event::DocumentStart(_) => {
                self.documents += 1;
                if self.documents > MAX_DOCUMENTS {
                    return Err(ParseError::ResourceLimit("document count"));
                }
                self.anchors.clear();
            }
            Event::MappingStart(id, tag) | Event::SequenceStart(id, tag) => {
                if self.parents.len() >= MAX_DEPTH {
                    return Err(ParseError::ResourceLimit("collection nesting depth"));
                }
                let node = Node::new(0, tag.as_deref());
                self.charge(node.cost)?;
                self.parents.push((*id, node));
            }
            Event::MappingEnd | Event::SequenceEnd => {
                if let Some((id, node)) = self.parents.pop() {
                    self.finish(id, node)?;
                }
            }
            Event::Scalar(text, _, id, tag) => {
                let node = Node::new(text.len(), tag.as_deref());
                self.charge(node.cost)?;
                self.finish(*id, node)?;
            }
            Event::Alias(id) => {
                let node = self
                    .anchors
                    .get(id)
                    .copied()
                    .ok_or(ParseError::UnresolvedAlias)?;
                self.charge(node.cost)?;
                self.finish(0, node)?;
            }
            Event::Nothing | Event::StreamStart | Event::StreamEnd | Event::DocumentEnd => {}
        }
        Ok(())
    }
}

pub(crate) fn load<'a>(
    input: &'a str,
    mut receive: impl FnMut(Event<'a>, Span) -> ParseResult<()>,
) -> ParseResult<()> {
    if input.len() > MAX_INPUT_BYTES {
        return Err(ParseError::ResourceLimit("input bytes"));
    }
    let mut budget = Budget::default();
    for result in Parser::new(BufferedInput::new(input.chars())) {
        let (event, span) = result?;
        budget.observe(&event)?;
        receive(event, span)?;
    }
    Ok(())
}
