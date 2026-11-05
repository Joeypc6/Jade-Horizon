// RuleList.populateChildren: the same search, and gamerule.jade.* sorts first in its category.
fn jade_rule_rows(screen: &RulesScreen, value: Vec<Row>) -> Vec<Row> {
    let mut rows = value;
    let rule: &'static Rule = &gamerules::hz_jade_rule::RULE;
    let filter = screen.search.value.to_lowercase();
    let has = |s: &str| s.to_lowercase().contains(&filter);
    let key = name_key(rule);
    let description = format!("{key}.description");
    let matches = has(rule.name)
        || has(&lang::get(&key))
        || has(&lang::get(&category_key(rule.category)))
        || (lang::has(&description) && has(&lang::get(&description)));
    if !matches {
        return rows;
    }
    match rows.iter().position(|row| matches!(row, Row::Category(c) if *c == rule.category)) {
        Some(at) => rows.insert(at + 1, Row::Rule(rule)),
        None => {
            let at = rows
                .iter()
                .position(|row| matches!(row, Row::Category(c) if *c > rule.category))
                .unwrap_or(rows.len());
            rows.insert(at, Row::Category(rule.category));
            rows.insert(at + 1, Row::Rule(rule));
        }
    }
    rows
}
