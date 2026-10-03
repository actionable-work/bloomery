use crate::files::expand_globs;
use bloomery_model::{Diagnostic, Evidence, SourceLocation};
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

pub fn scan(
    root: &Path,
    patterns: &[String],
    tag_prefix: &str,
) -> Result<Vec<Evidence>, Vec<Diagnostic>> {
    let paths = match expand_globs(root, patterns) {
        Ok(paths) => paths,
        Err(error) => return Err(vec![Diagnostic::new("ScannerError", error)]),
    };
    let mut evidence = Vec::new();
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
        let parsed = Parser::new(
            &allocator,
            &contents,
            SourceType::default().with_typescript(true),
        )
        .parse();
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
            evidence: Vec::new(),
        };
        visitor.visit_program(&parsed.program);
        evidence.extend(visitor.evidence);
    }
    if diagnostics.is_empty() {
        Ok(evidence)
    } else {
        Err(diagnostics)
    }
}

struct PlaywrightVisitor<'source> {
    source: &'source str,
    path: &'source Path,
    tag_prefix: &'source str,
    evidence: Vec<Evidence>,
}

impl<'ast> Visit<'ast> for PlaywrightVisitor<'_> {
    fn visit_call_expression(&mut self, expression: &CallExpression<'ast>) {
        if is_test_call(&expression.callee) {
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
                        self.evidence.push(Evidence {
                            id: id.to_owned(),
                            location: SourceLocation::new(
                                self.path,
                                Some(line_for_offset(self.source, literal.span.start)),
                            ),
                            scanner: "playwright",
                        });
                    }
                }
            }
        }
        self.visit_expression(&expression.callee);
        self.visit_arguments(&expression.arguments);
    }
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
