//! Compare mapping keys before a value loader can overwrite them. Node IDs are scoped to a
//! document. Aliases reuse the target ID; mapping identity is independent of entry order.
use crate::error::Location;
use indexmap::IndexSet;
use std::collections::{BTreeMap, HashMap};

use saphyr_parser::{Event, Marker, Tag};

use crate::{
    ParseError, ParseResult, Value,
    parser::{is_merge_scalar, normalize_representation},
};

type Id = usize;

#[derive(Eq, PartialEq, Hash)]
enum Node {
    Merge,
    Scalar(Value),
    Sequence(Option<Tag>, Vec<Id>),
    Mapping(Option<Tag>, Vec<(Id, Id)>),
}

struct Collection {
    mark: Marker,
    anchor: usize,
    tag: Option<Tag>,
    // Only keys, anchors and their descendants need a reconstructible identity.
    children: Option<Vec<Id>>,
    keys: Option<HashMap<Id, Marker>>,
    expecting_key: bool,
}

#[derive(Default)]
pub(crate) struct Keys {
    stack: Vec<Collection>,
    nodes: IndexSet<Node>,
    anchors: HashMap<usize, Id>,
}

impl Keys {
    fn capturing(&self, anchor: usize) -> bool {
        anchor != 0
            || self
                .stack
                .last()
                .is_some_and(|c| c.children.is_some() || c.expecting_key)
    }

    fn intern(&mut self, node: Node) -> Id {
        self.nodes.insert_full(node).0
    }

    fn mapping(&self, children: &[Id]) -> ParseResult<Vec<(Id, Id)>> {
        let mut pairs: Vec<_> = children
            .as_chunks::<2>()
            .0
            .iter()
            .map(|&[key, value]| (key, value))
            .collect();
        let Some(index) = pairs
            .iter()
            .position(|&(k, _)| matches!(&self.nodes[k], Node::Merge))
        else {
            pairs.sort_unstable();
            return Ok(pairs);
        };
        // Compare a mapping used as a key by its effective entries, including inherited ones.
        let (_, source) = pairs.swap_remove(index);
        let mut effective: BTreeMap<_, _> = pairs.into_iter().collect();
        let sources = match &self.nodes[source] {
            Node::Sequence(_, items) => items.as_slice(),
            _ => std::slice::from_ref(&source),
        };
        for &id in sources {
            let Node::Mapping(_, entries) = &self.nodes[id] else {
                return Err(ParseError::InvalidMergeValue {
                    kind: "non-mapping",
                });
            };
            for &(key, value) in entries {
                effective.entry(key).or_insert(value);
            }
        }
        Ok(effective.into_iter().collect())
    }

    fn finish(
        &mut self,
        id: Option<Id>,
        anchor: usize,
        mark: Marker,
        input: &str,
    ) -> ParseResult<()> {
        if anchor != 0 {
            self.anchors
                .insert(anchor, id.expect("anchors are captured"));
        }
        // The merge spelling is a directive only in key position. As an element or
        // mapping value inside a compound key it compares as the literal string.
        let id = if id.is_some_and(|id| matches!(self.nodes[id], Node::Merge))
            && self
                .stack
                .last()
                .is_some_and(|p| p.children.is_some() && !p.expecting_key)
        {
            Some(
                self.intern(Node::Scalar(Value::Value(crate::ScalarOwned::String(
                    "<<".into(),
                )))),
            )
        } else {
            id
        };
        if let Some(parent) = self.stack.last_mut() {
            if let Some(keys) = &mut parent.keys {
                if parent.expecting_key
                    && let Some(first) = keys.insert(id.expect("keys are captured"), mark)
                {
                    return Err(ParseError::DuplicateKey {
                        first: Location::marker(input, first),
                        duplicate: Location::marker(input, mark),
                    });
                }
                parent.expecting_key = !parent.expecting_key;
            }
            if let Some(children) = &mut parent.children {
                children.push(id.expect("children of a captured node are captured"));
            }
        }
        Ok(())
    }

    pub(crate) fn observe(
        &mut self,
        event: &Event<'_>,
        mark: Marker,
        input: &str,
    ) -> ParseResult<()> {
        match event {
            Event::DocumentStart(_) => {
                self.nodes.clear();
                self.anchors.clear();
            }
            Event::MappingStart(anchor, tag) | Event::SequenceStart(anchor, tag) => {
                let mapping = matches!(event, Event::MappingStart(..));
                let capture = self.capturing(*anchor);
                self.stack.push(Collection {
                    mark,
                    anchor: *anchor,
                    tag: tag.as_deref().cloned().filter(|t| {
                        !(t.is_yaml_core_schema()
                            && t.suffix == if mapping { "map" } else { "seq" })
                    }),
                    children: capture.then(Vec::new),
                    keys: mapping.then(HashMap::new),
                    expecting_key: mapping,
                });
            }
            Event::SequenceEnd | Event::MappingEnd => {
                let frame = self.stack.pop().expect("balanced parser events");
                let id = if let Some(children) = frame.children {
                    let node = if frame.keys.is_some() {
                        Node::Mapping(frame.tag, self.mapping(&children)?)
                    } else {
                        Node::Sequence(frame.tag, children)
                    };
                    Some(self.intern(node))
                } else {
                    None
                };
                self.finish(id, frame.anchor, frame.mark, input)?;
            }
            Event::Scalar(text, style, anchor, tag) => {
                let id = if self.capturing(*anchor) {
                    let node = if is_merge_scalar(text, *style, tag.as_deref()) {
                        Node::Merge
                    } else {
                        Node::Scalar(normalize_representation(
                            text.to_string(),
                            *style,
                            tag.as_deref().cloned(),
                        )?)
                    };
                    Some(self.intern(node))
                } else {
                    None
                };
                self.finish(id, *anchor, mark, input)?;
            }
            Event::Alias(anchor) => {
                let id = self
                    .anchors
                    .get(anchor)
                    .copied()
                    .ok_or(ParseError::UnresolvedAlias)?;
                self.finish(Some(id), 0, mark, input)?;
            }
            _ => {}
        }
        Ok(())
    }
}
