//! Plugin pane logic that doesn't need a browser: which tags a category and search show, and
//! the edits users make to an editable tag plugin (each returns the new category list to save).

pub use wordfabrica_remote::{Plugin, PluginKind, Tag, TagCategory};

/// Tags grouped by category: `(category index, [(tag index, tag)])`.
pub type TagGroups<'a> = Vec<(usize, Vec<(usize, &'a Tag)>)>;

/// The tags to list for `category` (`None` = all) and search `query`, grouped by category in
/// file order. The query matches the tag's label, text or note, or its category's name,
/// ignoring case; empty groups are left out.
pub fn filter_tags<'a>(categories: &'a [TagCategory], category: Option<usize>, query: &str) -> TagGroups<'a> {
    let q = query.trim().to_lowercase();
    let has = |s: &str| s.to_lowercase().contains(&q);
    categories
        .iter()
        .enumerate()
        .filter(|(i, _)| category.is_none_or(|c| c == *i))
        .filter_map(|(i, cat)| {
            let whole = q.is_empty() || has(&cat.name);
            let tags: Vec<(usize, &Tag)> =
                cat.tags.iter().enumerate().filter(|(_, t)| whole || has(&t.label) || has(&t.text) || has(&t.note)).collect();
            (!tags.is_empty()).then_some((i, tags))
        })
        .collect()
}

/// Add `tag` to the category named `category` (a new one at the end if there's none), or with
/// `original` = `(category, tag)` replace that tag: in place when its category stays the same,
/// otherwise moved to the end of the new category.
pub fn upsert_tag(categories: &[TagCategory], original: Option<(usize, usize)>, category: &str, tag: Tag) -> Vec<TagCategory> {
    let mut cats = categories.to_vec();
    let category = category.trim();
    if let Some((ci, ti)) = original
        && let Some(cat) = cats.get_mut(ci)
        && ti < cat.tags.len()
    {
        if cat.name == category {
            if let Some(slot) = cat.tags.get_mut(ti) {
                *slot = tag;
            }
            return cats;
        }
        cat.tags.remove(ti);
    }
    match cats.iter_mut().find(|c| c.name == category) {
        Some(cat) => cat.tags.push(tag),
        None => cats.push(TagCategory { name: category.to_string(), tags: vec![tag] }),
    }
    cats
}

/// Remove one tag (out-of-range indices change nothing).
pub fn delete_tag(categories: &[TagCategory], category: usize, tag: usize) -> Vec<TagCategory> {
    let mut cats = categories.to_vec();
    if let Some(cat) = cats.get_mut(category)
        && tag < cat.tags.len()
    {
        cat.tags.remove(tag);
    }
    cats
}

/// Remove a category and its tags.
pub fn delete_category(categories: &[TagCategory], category: usize) -> Vec<TagCategory> {
    let mut cats = categories.to_vec();
    if category < cats.len() {
        cats.remove(category);
    }
    cats
}

/// Rename a category. Renaming it to another category's name merges the two (tags appended).
pub fn rename_category(categories: &[TagCategory], category: usize, name: &str) -> Vec<TagCategory> {
    let mut cats = categories.to_vec();
    let name = name.trim();
    let Some(cat) = cats.get(category) else { return cats };
    match cats.iter().position(|c| c.name == name) {
        Some(other) if other != category => {
            let moved = cat.tags.clone();
            if let Some(target) = cats.get_mut(other) {
                target.tags.extend(moved);
            }
            cats.remove(category);
        }
        _ => {
            if let Some(cat) = cats.get_mut(category) {
                cat.name = name.to_string();
            }
        }
    }
    cats
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tag(label: &str, text: &str, note: &str) -> Tag {
        Tag { label: label.into(), text: text.into(), note: note.into() }
    }

    fn sample() -> Vec<TagCategory> {
        vec![
            TagCategory {
                name: "客户".into(), tags: vec![tag("客户名称", "{{customer}}", ""), tag("联系人", "{{contact}}", "Primary contact")]
            },
            TagCategory { name: "Dates".into(), tags: vec![tag("签订日期", "{{signDate}}", "")] },
            TagCategory { name: "Empty".into(), tags: vec![] },
        ]
    }

    /// Category names and tag labels, for compact assertions.
    fn shape(cats: &[TagCategory]) -> Vec<(String, Vec<String>)> {
        cats.iter().map(|c| (c.name.clone(), c.tags.iter().map(|t| t.label.clone()).collect())).collect()
    }

    #[test]
    fn categories_and_search_narrow_the_list() {
        let cats = sample();
        let labels = |r: TagGroups| r.into_iter().map(|(i, t)| (i, t.iter().map(|(_, t)| t.label.clone()).collect::<Vec<_>>())).collect::<Vec<_>>();
        assert_eq!(labels(filter_tags(&cats, None, "")), vec![(0, vec!["客户名称".into(), "联系人".into()]), (1, vec!["签订日期".into()])]);
        assert_eq!(labels(filter_tags(&cats, Some(1), "")), vec![(1, vec!["签订日期".into()])]);
        assert_eq!(labels(filter_tags(&cats, None, "名称")), vec![(0, vec!["客户名称".into()])], "label");
        assert_eq!(labels(filter_tags(&cats, None, "SIGNDATE")), vec![(1, vec!["签订日期".into()])], "text, any case");
        assert_eq!(labels(filter_tags(&cats, None, "primary")), vec![(0, vec!["联系人".into()])], "note");
        assert_eq!(labels(filter_tags(&cats, None, " dates ")), vec![(1, vec!["签订日期".into()])], "category name");
        assert!(filter_tags(&cats, Some(0), "signDate").is_empty(), "search stays inside the category");
        assert!(filter_tags(&cats, Some(9), "").is_empty());
        assert_eq!(filter_tags(&cats, None, "联系")[0].1[0].0, 1, "tag indices point into the category");
    }

    #[test]
    fn adding_and_editing_tags() {
        let cats = sample();
        let added = upsert_tag(&cats, None, " Dates ", tag("到期日", "", ""));
        assert_eq!(shape(&added)[1].1, vec!["签订日期", "到期日"], "existing category, trimmed name");
        let new_cat = upsert_tag(&cats, None, "项目", tag("项目名称", "", ""));
        assert_eq!(shape(&new_cat)[3], ("项目".into(), vec!["项目名称".into()]), "new category at the end");
        let edited = upsert_tag(&cats, Some((0, 0)), "客户", tag("客户全称", "{{customer}}", ""));
        assert_eq!(shape(&edited)[0].1, vec!["客户全称", "联系人"], "edited in place");
        let moved = upsert_tag(&cats, Some((0, 0)), "Dates", tag("客户名称", "", ""));
        assert_eq!(shape(&moved)[0].1, vec!["联系人"]);
        assert_eq!(shape(&moved)[1].1, vec!["签订日期", "客户名称"], "moved to the new category");
        let stale = upsert_tag(&cats, Some((7, 7)), "客户", tag("x", "", ""));
        assert_eq!(shape(&stale)[0].1.len(), 3, "a stale reference adds instead of panicking");
    }

    #[test]
    fn deleting_and_renaming() {
        let cats = sample();
        assert_eq!(shape(&delete_tag(&cats, 0, 1))[0].1, vec!["客户名称"]);
        assert_eq!(delete_tag(&cats, 0, 9), cats);
        assert_eq!(delete_tag(&cats, 9, 0), cats);
        assert_eq!(shape(&delete_category(&cats, 1)).iter().map(|c| c.0.as_str()).collect::<Vec<_>>(), vec!["客户", "Empty"]);
        assert_eq!(delete_category(&cats, 9), cats);
        assert_eq!(shape(&rename_category(&cats, 1, " 日期 "))[1].0, "日期");
        let merged = rename_category(&cats, 1, "客户");
        assert_eq!(shape(&merged), vec![("客户".into(), vec!["客户名称".into(), "联系人".into(), "签订日期".into()]), ("Empty".into(), vec![])]);
        assert_eq!(rename_category(&cats, 9, "x"), cats);
    }
}
