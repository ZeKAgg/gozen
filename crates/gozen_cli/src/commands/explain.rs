use gozen_linter::{rules, shader_rules};

pub fn run(rule: &str) -> anyhow::Result<()> {
    let rule_lower = rule.trim().to_lowercase();

    // Search all rules dynamically rather than maintaining a stale list
    let all = rules::all_rules();
    for r in &all {
        let m = r.metadata();
        if m.id.to_lowercase().contains(&rule_lower) || m.name.to_lowercase().contains(&rule_lower)
        {
            print_metadata(m);
            return Ok(());
        }
    }

    let all_project = rules::all_project_rules();
    for r in &all_project {
        let m = r.metadata();
        if m.id.to_lowercase().contains(&rule_lower) || m.name.to_lowercase().contains(&rule_lower)
        {
            print_metadata(m);
            return Ok(());
        }
    }

    let all_shader = shader_rules::all_shader_rules();
    for r in &all_shader {
        let m = r.metadata();
        if m.id.to_lowercase().contains(&rule_lower) || m.name.to_lowercase().contains(&rule_lower)
        {
            print_metadata(m);
            return Ok(());
        }
    }

    anyhow::bail!("Unknown rule: {}", rule);
}

fn print_metadata(m: &gozen_linter::rule::RuleMetadata) {
    println!("Rule: {}", m.id);
    println!("Group: {}", m.group);
    println!("Default severity: {:?}", m.default_severity);
    println!("Has fix: {}", m.has_fix);
    println!("\n{}\n", m.description);
    println!("{}", m.explanation);
}
