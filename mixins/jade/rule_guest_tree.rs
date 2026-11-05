// What players are sent comes from vanilla's commands report, which has no jade: rule.
fn jade_rule_guest_tree(target: usize, path: &Vec<String>, nodes: &mut Vec<Node>, paths: &mut HashMap<Vec<String>, usize>) {
    if path.len() != 1 || path[0] != "gamerule" {
        return;
    }
    let rule = &crate::gamerules::hz_jade_rule::RULE;
    let literal = nodes.len();
    nodes.push(Node {
        name: rule.name.to_owned(),
        kind: 1,
        executable: true,
        restricted: false,
        children: vec![literal + 1],
        redirect: None,
        parser: None,
        suggestions: None,
    });
    nodes.push(Node {
        name: "value".to_owned(),
        kind: 2,
        executable: true,
        restricted: false,
        children: Vec::new(),
        redirect: None,
        parser: Some(("brigadier:integer".to_owned(), serde_json::json!({ "min": rule.min, "max": rule.max }))),
        suggestions: None,
    });
    nodes[target].children.push(literal);
    let mut at = path.clone();
    at.push(rule.name.to_owned());
    paths.insert(at.clone(), literal);
    at.push("value".to_owned());
    paths.insert(at, literal + 1);
}
