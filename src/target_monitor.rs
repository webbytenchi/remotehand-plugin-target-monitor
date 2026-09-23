fn valid_hex_color(value: &str) -> bool {
    value.len() == 7
        && value.starts_with('#')
        && value.as_bytes()[1..].iter().all(|byte| byte.is_ascii_hexdigit())
}

fn validate_targets(targets: &[MonitorTarget]) -> Result<(), String> {
    if targets.len() > 64 {
        return Err("maximum of 64 monitoring targets".to_string());
    }
    let mut ids = std::collections::BTreeSet::new();
    for target in targets {
        if target.id.trim().is_empty() || target.id.len() > 128 {
            return Err("target id is missing or too long".to_string());
        }
        if !ids.insert(target.id.clone()) {
            return Err(format!("duplicate target id: {}", target.id));
        }
        if target.name.trim().is_empty() || target.name.len() > 128 {
            return Err("target name is missing or too long".to_string());
        }
        resolve_target(&target.address)?;
        if !valid_hex_color(&target.graph_normal_color)
            || !valid_hex_color(&target.graph_high_color)
            || !valid_hex_color(&target.graph_offline_color)
            || !valid_hex_color(&target.target_name_color)
            || !valid_hex_color(&target.latency_color)
            || !valid_hex_color(&target.background_color)
            || !valid_hex_color(&target.border_color)
        {
            return Err("target colors must use #RRGGBB format".to_string());
        }
    }
    Ok(())
}

include!(concat!(env!("OUT_DIR"), "/main_patched.rs"));
