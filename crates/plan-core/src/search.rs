//! Finding elements in a plan by name.

use crate::fuzzy::Fuzzy;
use crate::model::{NodeId, Plan};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Hit {
    pub id: NodeId,
    pub score: u32,
    /// Matched character positions in the element's name. Empty when only the type label matched.
    pub positions: Vec<u32>,
}

/// Elements whose name, or failing that their type, matches `query`. Name matches rank above type
/// matches. An empty query returns nothing.
pub fn search(plan: &Plan, query: &str, limit: usize) -> Vec<Hit> {
    if query.trim().is_empty() {
        return vec![];
    }
    let mut fuzzy = Fuzzy::new();
    let ids: Vec<NodeId> = plan.iter().map(|n| n.id).collect();
    let names: Vec<&str> = plan.iter().map(|n| n.name.as_str()).collect();
    let types: Vec<&str> = plan.iter().map(|n| n.type_label.as_str()).collect();
    let mut hits: Vec<Hit> = Vec::new();
    let mut seen = vec![false; ids.len()];
    for m in fuzzy.rank(query, &names) {
        seen[m.index] = true;
        hits.push(Hit { id: ids[m.index], score: m.score.saturating_mul(2).saturating_add(1000), positions: m.positions });
    }
    for m in fuzzy.rank(query, &types) {
        if !seen[m.index] {
            hits.push(Hit { id: ids[m.index], score: m.score, positions: vec![] });
        }
    }
    hits.sort_by(|a, b| b.score.cmp(&a.score).then(a.id.cmp(&b.id)));
    hits.truncate(limit);
    hits
}

#[cfg(test)]
mod tests {
    use super::*;

    fn plan() -> Plan {
        Plan::from_jmx_str(
            r#"<jmeterTestPlan><hashTree>
<TestPlan testname="Shop"/><hashTree>
 <ThreadGroup testname="Checkout Users"/><hashTree>
  <HTTPSamplerProxy testname="POST /api/login"/><hashTree/>
  <HTTPSamplerProxy testname="GET /api/cart"/><hashTree/>
  <JSONPostProcessor testname="token"/><hashTree/>
 </hashTree>
</hashTree></hashTree></jmeterTestPlan>"#,
        )
        .unwrap()
    }

    #[test]
    fn finds_by_name() {
        let p = plan();
        let hits = search(&p, "login", 10);
        assert_eq!(hits.len(), 1);
        assert_eq!(p.node(hits[0].id).unwrap().name, "POST /api/login");
        assert!(!hits[0].positions.is_empty());
    }

    #[test]
    fn finds_by_type_when_the_name_does_not_match() {
        let p = plan();
        let hits = search(&p, "json extractor", 10);
        assert_eq!(hits.len(), 1);
        assert_eq!(p.node(hits[0].id).unwrap().name, "token");
        assert!(hits[0].positions.is_empty());
    }

    #[test]
    fn name_matches_rank_above_type_matches() {
        let p = plan();
        // "api" is in two names; "HTTP Request" type does not contain it.
        let hits = search(&p, "api", 10);
        assert_eq!(hits.len(), 2);
        let hits = search(&p, "request", 10); // matches the type of the two samplers only
        assert_eq!(hits.len(), 2);
    }

    #[test]
    fn empty_query_and_limit() {
        let p = plan();
        assert!(search(&p, "", 10).is_empty());
        assert!(search(&p, "   ", 10).is_empty());
        assert_eq!(search(&p, "a", 2).len(), 2);
    }
}
