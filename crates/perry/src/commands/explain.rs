//! Explain command - explain error codes

use anyhow::{anyhow, Result};
use clap::Args;

use crate::OutputFormat;

#[derive(Args, Debug)]
pub struct ExplainArgs {
    /// Error code to explain (e.g., U001, D002)
    pub code: String,
}

struct ErrorExplanation {
    code: &'static str,
    title: &'static str,
    description: &'static str,
    example: Option<&'static str>,
    suggestion: Option<&'static str>,
    related: &'static [&'static str],
}

const ERROR_EXPLANATIONS: &[ErrorExplanation] = &[
    // Parse errors
    ErrorExplanation {
        code: "P001",
        title: "Parse Error",
        description: "The TypeScript parser encountered invalid syntax that could not be parsed.",
        example: Some("let x: number = ;  // Missing value"),
        suggestion: Some("Check for syntax errors like missing semicolons, brackets, or values."),
        related: &[],
    },
    // Type errors
    ErrorExplanation {
        code: "T001",
        title: "Type Mismatch",
        description: "The types of values in an operation are incompatible.",
        example: Some("const x: number = \"hello\";  // string assigned to number"),
        suggestion: Some("Ensure the assigned value matches the declared type."),
        related: &["T002", "T003"],
    },
    ErrorExplanation {
        code: "T002",
        title: "Missing Type Annotation",
        description: "A type annotation is recommended but not provided. While the code may compile, explicit types improve optimization.",
        example: Some("function add(a, b) { return a + b; }  // Missing parameter types"),
        suggestion: Some("Add explicit type annotations: function add(a: number, b: number): number"),
        related: &["T003", "T004"],
    },
    ErrorExplanation {
        code: "T003",
        title: "'any' Type Usage",
        description: "The 'any' type bypasses type checking and may prevent optimizations. Native compilation works best with specific types.",
        example: Some("let data: any = getValue();"),
        suggestion: Some("Replace 'any' with a specific type or 'unknown' if the type is truly dynamic."),
        related: &["T002", "T004"],
    },
    ErrorExplanation {
        code: "T004",
        title: "Implicit 'any'",
        description: "A variable or parameter has an implicit 'any' type because no type annotation was provided and the type could not be inferred.",
        example: Some("function process(data) { ... }  // 'data' is implicitly 'any'"),
        suggestion: Some("Add an explicit type annotation to the parameter or variable."),
        related: &["T002", "T003"],
    },
    // Unsupported features
    ErrorExplanation {
        code: "U001",
        title: "Unsupported Binary Operator",
        description: "This binary operator is not supported in native compilation.",
        example: Some("if ('x' in obj) { ... }  // 'in' operator"),
        suggestion: Some("Use explicit property checks or Object.hasOwn() instead."),
        related: &["U002", "U006"],
    },
    ErrorExplanation {
        code: "U002",
        title: "Unsupported Unary Operator",
        description: "This unary operator is not supported in native compilation.",
        example: Some("delete obj.prop;  // 'delete' operator"),
        suggestion: Some("Consider using different patterns that don't require this operator."),
        related: &["U001", "U006"],
    },
    ErrorExplanation {
        code: "U006",
        title: "Unsupported Feature",
        description: "This TypeScript feature is not yet supported in native compilation.",
        example: None,
        suggestion: Some("Check the documentation for supported features and possible workarounds."),
        related: &["U001", "U002"],
    },
    // Dynamic code
    ErrorExplanation {
        code: "D001",
        title: "Dynamic Property Access",
        description: "Accessing object properties with a dynamic (non-constant) key may not compile correctly.",
        example: Some("const value = obj[dynamicKey];  // where dynamicKey is a variable"),
        suggestion: Some("Use a constant key or restructure code to use known property names."),
        related: &["D002", "D003"],
    },
    ErrorExplanation {
        code: "D002",
        title: "eval() Usage",
        description: r#"eval() executes code at runtime, which cannot be compiled ahead-of-time.

Perry compiles TypeScript to native machine code at build time. eval() would require
including an entire JavaScript runtime in the compiled binary, defeating the purpose of
native compilation."#,
        example: Some("eval(userCode);  // Cannot be compiled"),
        suggestion: Some(r#"Alternatives:
1. Use a configuration-driven approach instead of code generation
2. Pre-compile known code patterns into separate functions
3. Use a switch/case for a finite set of operations"#),
        related: &["D003", "D001"],
    },
    ErrorExplanation {
        code: "D003",
        title: "new Function() Usage",
        description: "new Function() creates functions from strings at runtime, which cannot be compiled ahead-of-time.",
        example: Some("const fn = new Function('x', 'return x + 1');"),
        suggestion: Some("Define the function statically or use a lookup table of pre-defined functions."),
        related: &["D002", "D001"],
    },
    ErrorExplanation {
        code: "D005",
        title: "Dynamic Import",
        description: r#"Dynamic import() loads modules at runtime based on a computed path.

Native executables resolve all imports at compile time. Dynamic imports would require
bundling all possible modules and including a runtime module loader."#,
        example: Some("const mod = await import(`./plugins/${name}.ts`);"),
        suggestion: Some(r#"Use static imports with conditional logic:

import * as pluginA from './plugins/a';
import * as pluginB from './plugins/b';

const plugins = { a: pluginA, b: pluginB };
const mod = plugins[name];"#),
        related: &["D002", "D001"],
    },
    // Compatibility warnings
    ErrorExplanation {
        code: "C002",
        title: "Loose Equality",
        description: "Loose equality (== or !=) performs type coercion which may behave unexpectedly in native code.",
        example: Some("if (x == null) { ... }  // Loose equality"),
        suggestion: Some("Use strict equality (=== or !==) for predictable behavior."),
        related: &["C001"],
    },
    // Resolution errors
    ErrorExplanation {
        code: "R001",
        title: "Undefined Variable",
        description: "Reference to a variable that has not been declared.",
        example: Some("console.log(undeclaredVar);"),
        suggestion: Some("Declare the variable before using it, or check for typos."),
        related: &["R002"],
    },
    ErrorExplanation {
        code: "R002",
        title: "Undefined Function",
        description: "Reference to a function that has not been declared.",
        example: Some("undeclaredFunction();"),
        suggestion: Some("Declare or import the function before calling it."),
        related: &["R001"],
    },
];

pub fn run(args: ExplainArgs, format: OutputFormat, use_color: bool) -> Result<()> {
    let code = args.code.to_uppercase();

    let explanation = ERROR_EXPLANATIONS
        .iter()
        .find(|e| e.code == code)
        .ok_or_else(|| anyhow!("Unknown error code: {}", code))?;

    match format {
        OutputFormat::Text => {
            if use_color {
                println!(
                    "\n{}: {}\n{}",
                    console::style(&code).bold().cyan(),
                    console::style(explanation.title).bold(),
                    "=".repeat(code.len() + explanation.title.len() + 2)
                );
            } else {
                println!(
                    "\n{}: {}\n{}",
                    code,
                    explanation.title,
                    "=".repeat(code.len() + explanation.title.len() + 2)
                );
            }

            println!("\n{}\n", explanation.description);

            if let Some(example) = explanation.example {
                if use_color {
                    println!("{}:", console::style("Example").bold());
                } else {
                    println!("Example:");
                }
                for line in example.lines() {
                    println!("  {}", line);
                }
                println!();
            }

            if let Some(suggestion) = explanation.suggestion {
                if use_color {
                    println!("{}:", console::style("Suggestion").bold().green());
                } else {
                    println!("Suggestion:");
                }
                for line in suggestion.lines() {
                    println!("  {}", line);
                }
                println!();
            }

            if !explanation.related.is_empty() {
                if use_color {
                    println!(
                        "{}: {}",
                        console::style("Related").dim(),
                        explanation.related.join(", ")
                    );
                } else {
                    println!("Related: {}", explanation.related.join(", "));
                }
            }
        }
        OutputFormat::Json => {
            let output = serde_json::json!({
                "code": explanation.code,
                "title": explanation.title,
                "description": explanation.description,
                "example": explanation.example,
                "suggestion": explanation.suggestion,
                "related": explanation.related,
            });
            println!("{}", serde_json::to_string_pretty(&output)?);
        }
    }

    Ok(())
}
