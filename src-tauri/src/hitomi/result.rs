use futures::future::join_all;
use indexmap::IndexSet;
use regex::Regex;

use super::{get_gallery_ids_for_query, get_gallery_ids_from_nozomi};

async fn create_get_results_tasks(
    sort_by_popularity: bool,
    positive_terms: &[String],
) -> anyhow::Result<IndexSet<i32>> {
    if sort_by_popularity {
        get_gallery_ids_from_nozomi(None, "popular", "all").await
    } else if positive_terms.is_empty() {
        get_gallery_ids_from_nozomi(None, "index", "all").await
    } else {
        Ok(IndexSet::new())
    }
}

pub async fn do_search(query: String, sort_by_popularity: bool) -> anyhow::Result<IndexSet<i32>> {
    let terms: Vec<String> = query
        .trim()
        .strip_prefix('?')
        .unwrap_or(&query)
        .to_lowercase()
        .split_whitespace()
        .map(|s| s.replace('_', " "))
        .collect();

    let mut positive_terms = Vec::new();
    let mut negative_terms = Vec::new();
    let negative_pattern = Regex::new(r"^-")?;

    for term in terms {
        if negative_pattern.is_match(&term) {
            negative_terms.push(negative_pattern.replace(&term, "").to_string());
        } else if !term.is_empty() {
            positive_terms.push(term);
        }
    }

    let get_results_tasks = create_get_results_tasks(sort_by_popularity, &positive_terms);

    let get_positive_results_tasks: Vec<_> = positive_terms
        .iter()
        .map(|term| async move {
            get_gallery_ids_for_query(term)
                .await
                .unwrap_or_else(|_| IndexSet::new())
        })
        .collect();

    let get_negative_results_tasks: Vec<_> = negative_terms
        .iter()
        .map(|term| async move {
            get_gallery_ids_for_query(term)
                .await
                .unwrap_or_else(|_| IndexSet::new())
        })
        .collect();

    let (results, positive_results, negative_results) = tokio::join!(
        get_results_tasks,
        join_all(get_positive_results_tasks),
        join_all(get_negative_results_tasks)
    );
    let mut results = results?;

    for new_results in positive_results {
        if results.is_empty() {
            results = new_results;
        } else {
            results.retain(|id| new_results.contains(id));
        }
    }

    for new_results in negative_results {
        results.retain(|id| !new_results.contains(id));
    }

    Ok(results)
}
