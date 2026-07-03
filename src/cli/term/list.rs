use super::*;

pub(super) fn handle_list(args: TermListArgs, ctx: &mut CommandCtx) -> Result<CommandOutcome> {
    let root = ctx.require_repo_path()?;
    let filter_scope = args.scope.as_deref().unwrap_or("all");

    let (terms, scope_map) = if let Some(project_name) = ctx.project() {
        let project_path = svc_util::resolve_project(root, Some(project_name), ctx.cwd())?;
        let mut merged = if filter_scope == "global" {
            Vec::new()
        } else {
            term_svc::list_terms(&project_path, args.filter.as_deref())?
        };
        let global_terms = if filter_scope == "project" {
            Vec::new()
        } else {
            term_svc::global::list_terms(root, args.filter.as_deref())?
        };
        let mut scope_map: std::collections::HashMap<String, &'static str> = std::collections::HashMap::new();
        for t in &merged {
            scope_map.insert(t.term.clone(), "project");
        }
        for t in global_terms {
            if !scope_map.contains_key(&t.term) {
                scope_map.insert(t.term.clone(), "global");
                merged.push(t);
            }
        }
        merged.sort_by(|a, b| a.term.cmp(&b.term));
        (merged, scope_map)
    } else {
        let terms = term_svc::global::list_terms(root, args.filter.as_deref())?;
        (terms, std::collections::HashMap::new())
    };

    // Apply new filters (AND semantics)
    let filtered_terms: Vec<&crate::model::term::Term> = terms
        .iter()
        .filter(|t| {
            // --tag filter: term must have at least one matching tag
            if !args.tag.is_empty() && !args.tag.iter().any(|tag| t.tags.contains(tag)) {
                return false;
            }
            // --alias filter: term must have at least one matching alias
            if !args.alias.is_empty() && !args.alias.iter().any(|alias| t.aliases.contains(alias)) {
                return false;
            }
            // --has-correction filter: term must have at least one correction
            if args.has_correction && t.corrections.is_empty() {
                return false;
            }
            true
        })
        .collect();

    let opts = ListOpts::from_flags(args.no_headers.no_headers, args.no_trunc.no_trunc)
        .with_repo_root(Some(root.to_path_buf()));

    match ctx.format() {
        Format::Json => {
            let items: Vec<serde_json::Value> = filtered_terms
                .iter()
                .map(|t| {
                    let mut v = serde_json::to_value(t).map_err(MfError::Json)?;
                    if let Some(obj) = v.as_object_mut() {
                        obj.insert("identity".to_string(), serde_json::Value::String(t.identity()));
                        if let Some(scope) = scope_map.get(&t.term) {
                            obj.insert("scope".to_string(), serde_json::Value::String((*scope).to_string()));
                        }
                    }
                    Ok(v)
                })
                .collect::<Result<Vec<_>>>()?;
            Ok(CommandOutcome::Success(json_collection("terms", items), Vec::new(), None))
        }
        Format::Text => {
            let mut rows = Vec::with_capacity(filtered_terms.len());
            for t in &filtered_terms {
                let def = t.definition.as_deref().unwrap_or("-").to_string();
                let alias_display = if t.aliases.is_empty() { "-".to_string() } else { t.aliases.join(", ") };
                let tags_display = if t.tags.is_empty() { "-".to_string() } else { t.tags.join(", ") };
                let corr_display =
                    if t.corrections.is_empty() { "-".to_string() } else { t.corrections.len().to_string() };
                rows.push(ListRow {
                    cells: vec![
                        ListCell::Text(t.term.clone()),
                        ListCell::Text(def),
                        ListCell::Text(alias_display),
                        ListCell::Text(tags_display),
                        ListCell::Text(corr_display),
                    ],
                });
            }
            let view = ListView {
                headers: &["TERM", "DEFINITION", "ALIASES", "TAGS", "CORRECTIONS"],
                rows,
                plural_noun: "terms",
            };
            Ok(CommandOutcome::Raw(render_text(&view, &opts), None))
        }
    }
}
