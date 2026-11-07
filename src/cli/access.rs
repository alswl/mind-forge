//! One command-tree policy for repository write cooperation. Unknown commands
//! take the write lock; only explicitly read-only leaves may skip it.
use clap::ArgMatches;

#[derive(Debug, PartialEq)]
pub enum Access {
    Read,
    Write,
    Recovery,
}

pub fn classify(matches: &ArgMatches) -> Access {
    let mut node = matches;
    let mut path = Vec::new();
    let mut dry_run = false;
    while let Some((name, child)) = node.subcommand() {
        path.push(name);
        dry_run |= child.try_get_one::<bool>("dry_run").ok().flatten().copied().unwrap_or(false);
        node = child;
    }
    if path.first() == Some(&"lifecycle")
        || (path.first() == Some(&"additional") && matches!(path.last(), Some(&"rename" | &"remove")))
    {
        return Access::Recovery;
    }
    if dry_run || node.try_get_one::<bool>("check").ok().flatten().copied().unwrap_or(false) {
        return Access::Read;
    }
    let Some(leaf) = path.last().copied() else { return Access::Read };
    if matches!(
        leaf,
        "list"
            | "show"
            | "status"
            | "search"
            | "trace"
            | "schema"
            | "default"
            | "terminal"
            | "version"
            | "completion"
            | "render"
    ) {
        return Access::Read;
    }
    if leaf == "lint" && !node.try_get_one::<bool>("fix").ok().flatten().copied().unwrap_or(false) {
        return Access::Read;
    }
    if path.len() == 1
        && matches!(
            leaf,
            "project" | "article" | "source" | "asset" | "term" | "prompt" | "thinking" | "publish" | "config"
        )
    {
        return Access::Read;
    }
    Access::Write
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cli::RootCli;
    use clap::CommandFactory;

    #[test]
    fn write_families_and_dry_runs_share_one_policy() {
        for args in [
            vec!["mind", "new", "a"],
            vec!["output", "new", "a", "b"],
            vec!["project", "index"],
            vec!["article", "index"],
            vec!["source", "sync"],
            vec!["source", "admin", "rebuild"],
            vec!["asset", "index"],
            vec!["term", "new", "a"],
            vec!["term", "fix"],
            vec!["project", "lint", "--fix"],
            vec!["build", "a"],
            vec!["publish", "run", "a"],
            vec!["config", "generate", "--out", "a.yaml"],
        ] {
            let parsed = RootCli::command().try_get_matches_from(std::iter::once("mf").chain(args.clone())).unwrap();
            assert_eq!(classify(&parsed), Access::Write, "{args:?}");
        }
        for args in [
            vec!["mind", "new", "a", "-n"],
            vec!["project", "lint"],
            vec!["source", "status"],
            vec!["mind", "list"],
            vec!["output", "show", "a"],
        ] {
            let parsed = RootCli::command().try_get_matches_from(std::iter::once("mf").chain(args.clone())).unwrap();
            assert_eq!(classify(&parsed), Access::Read, "{args:?}");
        }
    }
}
