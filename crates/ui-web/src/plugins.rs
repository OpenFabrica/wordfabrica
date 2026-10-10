//! Plugin pane logic that doesn't need a browser: which tags a category and search show.

pub use wordcraft_remote::{Plugin, PluginKind, Tag, TagCategory};

/// The tags to list for `category` (`None` = all) and search `query`, grouped by category in
/// file order. The query matches the tag's label, text or note, or its category's name,
/// ignoring case; empty groups are left out.
pub fn filter_tags<'a>(categories: &'a [TagCategory], category: Option<usize>, query: &str) -> Vec<(usize, Vec<&'a Tag>)> {
    let q = query.trim().to_lowercase();
    let has = |s: &str| s.to_lowercase().contains(&q);
    categories
        .iter()
        .enumerate()
        .filter(|(i, _)| category.is_none_or(|c| c == *i))
        .filter_map(|(i, cat)| {
            let whole = q.is_empty() || has(&cat.name);
            let tags: Vec<&Tag> = cat.tags.iter().filter(|t| whole || has(&t.label) || has(&t.text) || has(&t.note)).collect();
            (!tags.is_empty()).then_some((i, tags))
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tag(label: &str, text: &str, note: &str) -> Tag {
        Tag { label: label.into(), text: text.into(), note: note.into() }
    }

    #[test]
    fn categories_and_search_narrow_the_list() {
        let cats = vec![
            TagCategory {
                name: "客户".into(), tags: vec![tag("客户名称", "{{customer}}", ""), tag("联系人", "{{contact}}", "Primary contact")]
            },
            TagCategory { name: "Dates".into(), tags: vec![tag("签订日期", "{{signDate}}", "")] },
            TagCategory { name: "Empty".into(), tags: vec![] },
        ];
        let labels =
            |r: Vec<(usize, Vec<&Tag>)>| r.into_iter().map(|(i, t)| (i, t.iter().map(|t| t.label.clone()).collect::<Vec<_>>())).collect::<Vec<_>>();
        assert_eq!(labels(filter_tags(&cats, None, "")), vec![(0, vec!["客户名称".into(), "联系人".into()]), (1, vec!["签订日期".into()])]);
        assert_eq!(labels(filter_tags(&cats, Some(1), "")), vec![(1, vec!["签订日期".into()])]);
        assert_eq!(labels(filter_tags(&cats, None, "名称")), vec![(0, vec!["客户名称".into()])], "label");
        assert_eq!(labels(filter_tags(&cats, None, "SIGNDATE")), vec![(1, vec!["签订日期".into()])], "text, any case");
        assert_eq!(labels(filter_tags(&cats, None, "primary")), vec![(0, vec!["联系人".into()])], "note");
        assert_eq!(labels(filter_tags(&cats, None, " dates ")), vec![(1, vec!["签订日期".into()])], "category name");
        assert!(filter_tags(&cats, Some(0), "signDate").is_empty(), "search stays inside the category");
        assert!(filter_tags(&cats, Some(9), "").is_empty());
    }
}
