use crate::ScanOutput;
use crate::files::expand_globs;
use bloomery_model::{Diagnostic, Evidence, SourceLocation, TestSite};
use oxc_allocator::Allocator;
use oxc_ast::{
    Visit,
    ast::{
        Argument, ArrayExpressionElement, CallExpression, Expression, ObjectPropertyKind,
        PropertyKey, PropertyKind,
    },
};
use oxc_parser::Parser;
use oxc_span::SourceType;
use std::fs;
use std::path::Path;

#[cfg(test)]
use std::sync::atomic::{AtomicUsize, Ordering};

#[cfg(test)]
pub(crate) static PARSE_COUNT: AtomicUsize = AtomicUsize::new(0);

pub fn scan(
    root: &Path,
    patterns: &[String],
    tag_prefix: &str,
) -> Result<ScanOutput, Vec<Diagnostic>> {
    let paths = match expand_globs(root, patterns) {
        Ok(paths) => paths,
        Err(error) => return Err(vec![Diagnostic::new("ScannerError", error)]),
    };
    let mut output = ScanOutput::default();
    let mut diagnostics = Vec::new();
    for path in paths {
        let contents = match fs::read_to_string(&path) {
            Ok(contents) => contents,
            Err(error) => {
                diagnostics.push(
                    Diagnostic::new(
                        "PlaywrightScanError",
                        format!("Unable to read source: {error}"),
                    )
                    .at(path, Some(1)),
                );
                continue;
            }
        };

        let allocator = Allocator::default();
        let parsed = parse(&allocator, &contents);
        if !parsed.errors.is_empty() {
            diagnostics.push(
                Diagnostic::new(
                    "PlaywrightParseError",
                    format!(
                        "Unable to parse TypeScript source: {} syntax errors",
                        parsed.errors.len()
                    ),
                )
                .at(path, Some(1)),
            );
            continue;
        }

        let mut visitor = PlaywrightVisitor {
            source: &contents,
            path: &path,
            tag_prefix,
            output: ScanOutput::default(),
        };
        visitor.visit_program(&parsed.program);
        output.absorb(visitor.output);
    }
    if diagnostics.is_empty() {
        Ok(output)
    } else {
        Err(diagnostics)
    }
}

fn parse<'a>(allocator: &'a Allocator, contents: &'a str) -> oxc_parser::ParserReturn<'a> {
    #[cfg(test)]
    PARSE_COUNT.fetch_add(1, Ordering::Relaxed);
    Parser::new(
        allocator,
        contents,
        SourceType::default().with_typescript(true),
    )
    .parse()
}

struct PlaywrightVisitor<'source> {
    source: &'source str,
    path: &'source Path,
    tag_prefix: &'source str,
    output: ScanOutput,
}

impl<'ast> Visit<'ast> for PlaywrightVisitor<'_> {
    fn visit_call_expression(&mut self, expression: &CallExpression<'ast>) {
        if is_test_call(&expression.callee) {
            let mut references = Vec::new();
            for argument in &expression.arguments {
                let Some(options) = object_expression(argument) else {
                    continue;
                };
                for property in &options.properties {
                    let ObjectPropertyKind::ObjectProperty(property) = property else {
                        continue;
                    };
                    if property.kind != PropertyKind::Init
                        || !matches!(
                            &property.key,
                            PropertyKey::StaticIdentifier(key) if key.name == "tag"
                        )
                    {
                        continue;
                    }
                    let Expression::ArrayExpression(array) = &property.value else {
                        continue;
                    };
                    for element in &array.elements {
                        let ArrayExpressionElement::StringLiteral(literal) = element else {
                            continue;
                        };
                        let value = literal.value.as_ref();
                        let Some(id) = value.strip_prefix(self.tag_prefix) else {
                            continue;
                        };
                        let line = line_for_offset(self.source, literal.span.start);
                        self.output.evidence.push(Evidence {
                            id: id.to_owned(),
                            location: SourceLocation::new(self.path, Some(line)),
                            scanner: "playwright",
                        });
                        references.push(id.to_owned());
                    }
                }
            }
            let name = first_string_argument(&expression.arguments);
            let line = line_for_offset(self.source, expression.span.start);
            self.output.tests.push(TestSite {
                scanner: "playwright",
                name,
                location: SourceLocation::new(self.path, Some(line)),
                references,
            });
        }
        self.visit_expression(&expression.callee);
        self.visit_arguments(&expression.arguments);
    }
}

fn first_string_argument(arguments: &[Argument<'_>]) -> Option<String> {
    let first = arguments.first()?;
    let Argument::StringLiteral(literal) = first else {
        return None;
    };
    Some(literal.value.to_string())
}

fn is_test_call(callee: &Expression<'_>) -> bool {
    match callee {
        Expression::Identifier(identifier) => identifier.name == "test",
        Expression::StaticMemberExpression(member) => {
            member.property.name == "describe"
                && matches!(
                    &member.object,
                    Expression::Identifier(identifier) if identifier.name == "test"
                )
        }
        _ => false,
    }
}

fn object_expression<'ast>(
    argument: &'ast Argument<'ast>,
) -> Option<&'ast oxc_ast::ast::ObjectExpression<'ast>> {
    match argument {
        Argument::ObjectExpression(object) => Some(object),
        _ => None,
    }
}

fn line_for_offset(source: &str, offset: u32) -> usize {
    source[..offset as usize]
        .bytes()
        .filter(|byte| *byte == b'\n')
        .count()
        + 1
}
