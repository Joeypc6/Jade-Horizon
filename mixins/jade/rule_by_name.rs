fn jade_rule_by_name(rules: &GameRules, value: BTreeMap<String, i32>) -> BTreeMap<String, i32> {
    let mut out = value;
    out.insert(hz_jade_rule::NAME.to_owned(), rules.int(hz_jade_rule::NAME));
    out
}
