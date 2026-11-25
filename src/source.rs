//! Measure encoded source ranges before handing events to the value loader.
//! Saphyr's marker columns count Unicode scalars. Its marker index can drift
//! after a non-ASCII directive; convert line and column against the original text.
use crate::{Limits, ParseError, ParseResult, error::Location};
use fast_yaml_core::{OneBased, SourcePosition, position::ByteCursor};
use saphyr_parser::{Event, Marker, ScalarStyle, Span};

fn position(mark: Marker) -> SourcePosition {
    SourcePosition::new(
        OneBased::new(mark.line()).unwrap_or(OneBased::FIRST),
        OneBased::from_index(mark.col()),
    )
}

pub(crate) fn offset(input: &str, mark: Marker) -> usize {
    ByteCursor::new(input).offset(position(mark))
}

pub(crate) struct SourceGuard<'a> {
    input: &'a str,
    limits: &'a Limits,
    starts: ByteCursor<'a>,
    ends: ByteCursor<'a>,
    scanned: usize,
    document: usize,
    next_document: usize,
}

impl<'a> SourceGuard<'a> {
    pub(crate) fn new(input: &'a str, limits: &'a Limits) -> Option<Self> {
        (input.len() > limits.max_token_bytes || input.len() as u64 > limits.max_document_bytes)
            .then(|| Self {
                input,
                limits,
                starts: ByteCursor::new(input),
                ends: ByteCursor::new(input),
                scanned: 0,
                document: 0,
                next_document: 0,
            })
    }

    fn check(&self, start: usize, end: usize, max: u64, name: &'static str) -> ParseResult<()> {
        if end.saturating_sub(start) as u64 > max {
            return Err(ParseError::ResourceLimit {
                limit: name,
                at: Some(Location::at(self.input, start + max as usize)),
            });
        }
        Ok(())
    }

    fn token(&self, start: usize, end: usize) -> ParseResult<()> {
        self.check(
            start,
            end,
            self.limits.max_token_bytes as u64,
            "encoded token bytes",
        )
    }

    // Syntax outside scalar spans: skip punctuation, measure comments, properties
    // and directive words. Each call consumes a disjoint piece of the input.
    fn gap(&self, mut i: usize, end: usize) -> ParseResult<()> {
        let bytes = self.input.as_bytes();
        while i < end {
            let first = bytes[i];
            match first {
                b'#' => {
                    let stop = i + self.input[i..end].find(['\r', '\n']).unwrap_or(end - i);
                    self.token(i + 1, stop)?;
                    i = stop;
                }
                b'&' | b'*' | b'!' => {
                    i = self.property(i, end)?;
                }
                b'%' => {
                    // Directive names and arguments are separate lexical words.
                    i += 1;
                    let line_end = i + self.input[i..end].find(['\r', '\n']).unwrap_or(end - i);
                    while i < line_end {
                        if bytes[i] == b'#' {
                            break;
                        }
                        let start = i;
                        while i < line_end && !matches!(bytes[i], b' ' | b'\t') {
                            i += 1;
                        }
                        self.token(start, i)?;
                        while i < line_end && matches!(bytes[i], b' ' | b'\t') {
                            i += 1;
                        }
                    }
                }
                _ => i += 1,
            }
        }
        Ok(())
    }

    fn property(&self, start: usize, end: usize) -> ParseResult<usize> {
        let raw = &self.input[start..end];
        if let Some(rest) = raw.strip_prefix("!<") {
            let length = rest.find('>').unwrap_or(rest.len());
            self.token(start + 2, start + 2 + length)?;
            return Ok((start + 3 + length).min(end));
        }
        let body = &raw[1..];
        let length = body
            .find([' ', '\t', '\r', '\n', ',', '[', ']', '{', '}', '\u{feff}'])
            .unwrap_or(body.len());
        self.token(start + 1, start + 1 + length)?;
        Ok(start + 1 + length)
    }

    fn scalar(&mut self, mut start: usize, end: usize, style: ScalarStyle) -> ParseResult<()> {
        let bytes = self.input.as_bytes();
        // A node span can include an anchor/tag and separating whitespace or comments.
        while start < end {
            match bytes[start] {
                b'&' | b'!' => start = self.property(start, end)?,
                b' ' | b'\r' | b'\t' | b'\n' => start += 1,
                b'#' => {
                    let stop = start
                        + self.input[start..end]
                            .find(['\r', '\n'])
                            .unwrap_or(end - start);
                    self.token(start + 1, stop)?;
                    start = stop;
                }
                _ => break,
            }
        }
        if start == end {
            self.scanned = end;
            return Ok(());
        }
        self.scanned = match style {
            ScalarStyle::Plain => {
                self.token(start, end)?;
                end
            }
            ScalarStyle::SingleQuoted | ScalarStyle::DoubleQuoted => {
                // Saphyr includes trailing whitespace and comments in quoted spans.
                let quote = bytes[start];
                let mut i = start + 1;
                while i < end {
                    if bytes[i] == quote {
                        if quote == b'\'' && bytes.get(i + 1) == Some(&quote) {
                            i += 2;
                        } else {
                            break;
                        }
                    } else if quote == b'"' && bytes[i] == b'\\' {
                        i += 2;
                    } else {
                        i += 1;
                    }
                }
                self.token(start + 1, i.min(end))?;
                (i + 1).min(end)
            }
            ScalarStyle::Folded | ScalarStyle::Literal => {
                let header_end = start
                    + self.input[start..end]
                        .find(['\r', '\n'])
                        .unwrap_or(end - start);
                self.gap(start + 1, header_end)?;
                let body = after_break(bytes, header_end, end);
                // Indentation consumed by the scanner on the following sibling's line
                // belongs to that sibling, not this scalar.
                let raw = &self.input[body..end];
                let tail = raw.rsplit_once(['\n', '\r']).map_or(raw, |(_, t)| t);
                let unused_indent = if end < self.input.len() && tail.bytes().all(|b| b == b' ') {
                    tail.len()
                } else {
                    0
                };
                self.token(body, end - unused_indent)?;
                end
            }
        };
        Ok(())
    }

    pub(crate) fn observe(&mut self, event: &Event<'_>, span: Span) -> ParseResult<()> {
        let start = self.starts.offset(position(span.start));
        if matches!(event, Event::DocumentStart(_)) {
            self.document = self.next_document;
        }
        let mut used = start;
        if matches!(event, Event::DocumentEnd) {
            self.next_document = start;
            // Explicit '...' and its own line terminator belong to the closing document.
            if self.input[start..].starts_with("...") {
                let end = start
                    + self.input[start..]
                        .find(['\r', '\n'])
                        .unwrap_or(self.input.len() - start);
                self.next_document = after_break(self.input.as_bytes(), end, self.input.len());
            }
            used = self.next_document;
        }
        if self.input.len() > self.limits.max_token_bytes
            && matches!(
                event,
                Event::Alias(_) | Event::SequenceStart(_, _) | Event::MappingStart(_, _)
            )
            && start >= self.scanned
        {
            self.gap(self.scanned, start)?;
            self.scanned = start;
            if matches!(event, Event::Alias(_))
                || matches!(event, Event::MappingStart(id, tag) | Event::SequenceStart(id, tag) if *id != 0 || tag.is_some())
            {
                let end = self.ends.offset(position(span.end));
                self.gap(start, end)?;
                self.scanned = end;
            }
        }
        if let Event::Scalar(text, style, _, _) = event
            && (!text.is_empty() || *style != ScalarStyle::Plain)
        {
            used = self.ends.offset(position(span.end));
            if self.input.len() > self.limits.max_token_bytes && start >= self.scanned {
                self.gap(self.scanned, start)?;
                self.scalar(start, used, *style)?;
            }
        }
        if matches!(event, Event::StreamEnd) {
            used = self.input.len();
            if self.input.len() > self.limits.max_token_bytes {
                self.gap(self.scanned, used)?;
            }
        }
        self.check(
            self.document,
            used,
            self.limits.max_document_bytes,
            "document bytes",
        )
    }
}

fn after_break(bytes: &[u8], i: usize, end: usize) -> usize {
    if i == end {
        return end;
    }
    if bytes[i] == b'\r' && bytes.get(i + 1) == Some(&b'\n') {
        i + 2
    } else {
        i + 1
    }
}
