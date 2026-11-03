//! How a run talks to the outside: the output mode and how much it says.

use std::io::IsTerminal;

use clap::{Args, ValueEnum};

/// The `--output` choice.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default, ValueEnum)]
pub enum OutputArg {
    /// `github` under GitHub Actions, `tty` on a terminal, `plain` otherwise.
    #[default]
    Auto,
    Tty,
    Plain,
    Github,
}

/// The resolved mode.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Mode {
    /// Event lines plus a live block of the running tasks at the bottom of the terminal.
    Tty,
    /// Event lines only.
    Plain,
    /// Foldable log groups, error annotations, progress lines.
    Github,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum Verbosity {
    /// Only failures and the summary.
    Quiet,
    #[default]
    Normal,
    /// The whole log of every task under its result line.
    Verbose,
}

/// The flags of every command that runs a task graph.
#[derive(Args, Clone, Default)]
pub struct OutputArgs {
    /// Output mode: auto, tty (live status block), plain (event lines), github (log groups, annotations).
    #[arg(long, value_enum, default_value_t = OutputArg::Auto)]
    pub output: OutputArg,
    /// Print the whole log of every task under its result line.
    #[arg(short, long, conflicts_with = "quiet")]
    pub verbose: bool,
    /// Print only failures and the summary.
    #[arg(short, long)]
    pub quiet: bool,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Output {
    pub mode: Mode,
    pub verbosity: Verbosity,
}

pub fn resolve(arg: OutputArg, github_actions: bool, terminal: bool) -> Mode {
    match arg {
        OutputArg::Tty => Mode::Tty,
        OutputArg::Plain => Mode::Plain,
        OutputArg::Github => Mode::Github,
        OutputArg::Auto if github_actions => Mode::Github,
        OutputArg::Auto if terminal => Mode::Tty,
        OutputArg::Auto => Mode::Plain,
    }
}

impl Output {
    #[cfg(test)]
    pub fn plain() -> Self {
        Self {
            mode: Mode::Plain,
            verbosity: Verbosity::Normal,
        }
    }

    pub fn from_args(args: &OutputArgs) -> Self {
        let github = std::env::var("GITHUB_ACTIONS").is_ok_and(|value| value == "true");
        let verbosity = if args.quiet {
            Verbosity::Quiet
        } else if args.verbose {
            Verbosity::Verbose
        } else {
            Verbosity::Normal
        };
        Self {
            mode: resolve(args.output, github, std::io::stdout().is_terminal()),
            verbosity,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn auto_prefers_github_then_terminal() {
        assert_eq!(resolve(OutputArg::Auto, true, true), Mode::Github);
        assert_eq!(resolve(OutputArg::Auto, false, true), Mode::Tty);
        assert_eq!(resolve(OutputArg::Auto, false, false), Mode::Plain);
    }

    #[test]
    fn explicit_choice_wins() {
        assert_eq!(resolve(OutputArg::Plain, true, true), Mode::Plain);
        assert_eq!(resolve(OutputArg::Tty, true, false), Mode::Tty);
        assert_eq!(resolve(OutputArg::Github, false, false), Mode::Github);
    }

    #[test]
    fn quiet_and_verbose_conflict() {
        use clap::Parser;
        #[derive(Parser)]
        struct Cli {
            #[command(flatten)]
            out: OutputArgs,
        }
        assert!(Cli::try_parse_from(["x", "-q", "-v"]).is_err());
        let cli = Cli::try_parse_from(["x", "--output", "github", "-q"]).unwrap();
        assert_eq!(cli.out.output, OutputArg::Github);
        assert_eq!(Output::from_args(&cli.out).verbosity, Verbosity::Quiet);
    }
}
