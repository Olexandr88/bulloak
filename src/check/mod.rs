//! Defines the `bulloak check` command.
//!
//! This command performs checks on the relationship between a bulloak tree and a
//! Solidity file.

use std::fs;
use std::path::PathBuf;

use clap::Parser;
use owo_colors::OwoColorize;
use violation::{Violation, ViolationKind};

use crate::check::violation::fix_order;
use crate::sol::find_contract;
use crate::utils::pluralize;

use self::context::Context;
use self::rules::Checker;

mod context;
mod location;
mod rules;
mod utils;
pub(crate) mod violation;

/// Check that the tests match the spec.
#[derive(Debug, Parser)]
pub struct Check {
    /// The set of tree files to use as spec.
    ///
    /// Solidity file names are inferred from the specs.
    files: Vec<PathBuf>,
    /// Whether to fix any issues found.
    #[arg(long, group = "fix-violations", default_value = "false")]
    fix: bool,
    /// When `--fix` is passed, use `--stdout` to direct output
    /// to standard output instead of writing to files.
    #[arg(long, requires = "fix-violations", default_value = "false")]
    stdout: bool,
}

impl Check {
    /// Entrypoint for `bulloak check`.
    ///
    /// Note that we don't deal with `solang_parser` errors at all.
    pub fn run(self) {
        let mut violations = Vec::new();
        let ctxs: Vec<Context> = self
            .files
            .iter()
            .filter_map(|tree_path| {
                Context::new(tree_path.clone())
                    .map_err(|violation| violations.push(violation))
                    .ok()
            })
            .collect();

        if self.fix {
            let mut fixed_count = 0;
            for mut ctx in ctxs {
                let violations = rules::structural_match::StructuralMatcher::check(&ctx);
                let fixable_count = violations.iter().filter(|v| v.is_fixable()).count();

                // Process violations that affect function order first.
                let violations = violations
                    .into_iter()
                    .filter(|v| !matches!(v.kind, ViolationKind::FunctionOrderMismatch(_, _, _)));
                for violation in violations {
                    ctx = violation.kind.fix(ctx);
                }

                // Second pass fixing order violations.
                let violations = rules::structural_match::StructuralMatcher::check(&ctx);
                let violations: Vec<Violation> = violations
                    .into_iter()
                    .filter(|v| matches!(v.kind, ViolationKind::FunctionOrderMismatch(_, _, _)))
                    .collect();
                if !violations.is_empty() {
                    if let Some(contract_sol) = find_contract(&ctx.pt) {
                        if let Some(contract_hir) = ctx.hir.clone().find_contract() {
                            ctx = fix_order(&violations, &contract_sol, contract_hir, ctx);
                        }
                    }
                }

        exit(&violations);

        Ok(())
                fixed_count += fixable_count;
            }

            let issue_literal = pluralize(fixed_count, "issue", "issues");
            println!(
                "\n{}: {} {} fixed.",
                "success".bold().green(),
                fixed_count,
                issue_literal
            );
        } else {
            for ctx in ctxs {
                violations.append(&mut rules::structural_match::StructuralMatcher::check(&ctx));
            }

            exit(&violations);
        }
    }
    }
}

fn exit(violations: &[Violation]) {
    if violations.is_empty() {
        println!(
            "{}",
            "All checks completed successfully! No issues found.".green()
        );
    } else {
        for violation in violations {
            eprint!("{violation}");
        }

        let pluralized_check = if violations.len() == 1 {
            "check"
        } else {
            "checks"
        };
        eprintln!(
            "\n{}: {} {} failed. See details above.",
            "error".bold().red(),
            violations.len(),
            pluralized_check
        );

        std::process::exit(1);
    }
}
