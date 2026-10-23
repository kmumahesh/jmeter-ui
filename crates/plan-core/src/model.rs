//! The plan as a tree of elements with stable ids.

use crate::catalog::{self, Category};
use anyhow::{Context, Result};
use jmx_model::Document;
use std::path::Path;

#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct NodeId(pub u32);

#[derive(Debug, Clone)]
pub struct Node {
    pub id: NodeId,
    pub parent: Option<NodeId>,
    pub children: Vec<NodeId>,
    /// XML tag, for example `HTTPSamplerProxy`, or a full class name for plugin elements.
    pub tag: String,
    pub testclass: String,
    pub guiclass: String,
    /// The name shown in the tree (the `testname` attribute).
    pub name: String,
    pub enabled: bool,
    pub category: Category,
    /// What kind of element this is, for example `HTTP Request`.
    pub type_label: String,
    /// Simple settings saved on the element, in file order.
    pub props: Vec<jmx_model::plan::Prop>,
}

/// Ids are the uids of the plan's XML elements (see `jmx_model::Element::uid`), so they stay the same
/// when the plan is edited and rebuilt. They are not consecutive.
#[derive(Debug, Clone, Default)]
pub struct Plan {
    /// In document order.
    nodes: Vec<Node>,
    index: std::collections::HashMap<u32, usize>,
    roots: Vec<NodeId>,
}

/// How many elements of each kind a plan holds.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Summary {
    pub total: usize,
    pub disabled: usize,
    pub unknown: usize,
    pub plugin: usize,
    pub by_category: Vec<(Category, usize)>,
}

impl Plan {
    pub fn from_jmx_str(src: &str) -> Result<Plan> {
        let mut doc = Document::parse(src)?;
        doc.index_plan();
        Plan::from_document(&doc)
    }

    pub fn load(path: &Path) -> Result<Plan> {
        let src = std::fs::read_to_string(path).with_context(|| format!("cannot read {}", path.display()))?;
        Plan::from_jmx_str(&src).with_context(|| format!("cannot open {} as a JMeter plan", path.display()))
    }

    /// Build the plan from a document whose elements have uids (`Document::index_plan`).
    pub fn from_document(doc: &Document) -> Result<Plan> {
        let top = jmx_model::plan::plan_tree(doc)?;
        if top.iter().any(|n| n.uid == 0) {
            let mut indexed = doc.clone();
            indexed.index_plan();
            return Plan::from_document(&indexed);
        }
        let mut plan = Plan::default();
        for n in &top {
            let id = plan.add(n, None);
            plan.roots.push(id);
        }
        Ok(plan)
    }

    fn add(&mut self, src: &jmx_model::plan::PlanNode, parent: Option<NodeId>) -> NodeId {
        let id = NodeId(src.uid);
        self.index.insert(src.uid, self.nodes.len());
        let (type_label, category) = catalog::describe(&src.tag, &src.guiclass);
        let name = if src.testname.is_empty() { type_label.clone() } else { src.testname.clone() };
        self.nodes.push(Node {
            id,
            parent,
            children: Vec::new(),
            tag: src.tag.clone(),
            testclass: src.testclass.clone(),
            guiclass: src.guiclass.clone(),
            name,
            enabled: src.enabled,
            category,
            type_label,
            props: src.props.clone(),
        });
        for c in &src.children {
            let cid = self.add(c, Some(id));
            let at = self.index[&id.0];
            self.nodes[at].children.push(cid);
        }
        id
    }

    pub fn roots(&self) -> &[NodeId] {
        &self.roots
    }

    pub fn len(&self) -> usize {
        self.nodes.len()
    }

    pub fn is_empty(&self) -> bool {
        self.nodes.is_empty()
    }

    pub fn node(&self, id: NodeId) -> Option<&Node> {
        self.index.get(&id.0).map(|&i| &self.nodes[i])
    }

    /// All nodes in document order.
    pub fn iter(&self) -> impl Iterator<Item = &Node> {
        self.nodes.iter()
    }

    /// Ids from the root down to `id`, inclusive.
    pub fn path_to(&self, id: NodeId) -> Vec<NodeId> {
        let mut out = Vec::new();
        let mut cur = Some(id);
        while let Some(c) = cur {
            out.push(c);
            cur = self.node(c).and_then(|n| n.parent);
        }
        out.reverse();
        out
    }

    pub fn depth(&self, id: NodeId) -> usize {
        self.path_to(id).len().saturating_sub(1)
    }

    /// Names from the root down to `id`, for a breadcrumb.
    pub fn breadcrumb(&self, id: NodeId) -> Vec<String> {
        self.path_to(id).into_iter().filter_map(|i| self.node(i)).map(|n| n.name.clone()).collect()
    }

    /// Number of elements below `id`.
    pub fn descendant_count(&self, id: NodeId) -> usize {
        self.node(id).map_or(0, |n| n.children.iter().map(|&c| 1 + self.descendant_count(c)).sum())
    }

    /// The first element whose name equals `name`.
    pub fn find_by_name(&self, name: &str) -> Option<NodeId> {
        self.nodes.iter().find(|n| n.name == name).map(|n| n.id)
    }

    pub fn summary(&self) -> Summary {
        let mut counts: std::collections::BTreeMap<Category, usize> = Default::default();
        let mut s = Summary { total: self.nodes.len(), ..Default::default() };
        for n in &self.nodes {
            *counts.entry(n.category).or_default() += 1;
            if !n.enabled {
                s.disabled += 1;
            }
            if n.category == Category::Unknown {
                s.unknown += 1;
            }
            if n.tag.contains('.') {
                s.plugin += 1;
            }
        }
        s.by_category = counts.into_iter().collect();
        s
    }

    /// Would moving or adding `node` under `new_parent` be allowed (category rules, no cycles)?
    pub fn can_move(&self, node: NodeId, new_parent: NodeId) -> bool {
        let (Some(n), Some(p)) = (self.node(node), self.node(new_parent)) else { return false };
        if node == new_parent || self.path_to(new_parent).contains(&node) {
            return false;
        }
        crate::rules::can_add(p.category, n.category)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SMALL: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
<jmeterTestPlan version="1.2" properties="5.0" jmeter="5.6.3">
  <hashTree>
    <TestPlan guiclass="TestPlanGui" testclass="TestPlan" testname="Shop" enabled="true"/>
    <hashTree>
      <ThreadGroup guiclass="ThreadGroupGui" testclass="ThreadGroup" testname="Users" enabled="true"/>
      <hashTree>
        <LoopController guiclass="LoopControlPanel" testclass="LoopController" testname="" enabled="true"/>
        <hashTree>
          <HTTPSamplerProxy guiclass="HttpTestSampleGui" testclass="HTTPSamplerProxy" testname="GET /" enabled="true"/>
          <hashTree>
            <ResponseAssertion guiclass="AssertionGui" testclass="ResponseAssertion" testname="ok" enabled="false"/>
            <hashTree/>
          </hashTree>
        </hashTree>
        <kg.apc.jmeter.samplers.DummySampler guiclass="x" testclass="y" testname="Dummy" enabled="true"/>
        <hashTree/>
      </hashTree>
      <ResultCollector guiclass="ViewResultsFullVisualizer" testclass="ResultCollector" testname="Tree" enabled="true"/>
      <hashTree/>
    </hashTree>
  </hashTree>
</jmeterTestPlan>"#;

    fn plan() -> Plan {
        Plan::from_jmx_str(SMALL).unwrap()
    }

    #[test]
    fn builds_the_tree_in_document_order() {
        let p = plan();
        assert_eq!(p.len(), 7);
        assert_eq!(p.roots().len(), 1);
        let names: Vec<_> = p.iter().map(|n| n.name.as_str()).collect();
        assert_eq!(names, ["Shop", "Users", "Loop Controller", "GET /", "ok", "Dummy", "Tree"]);
    }

    #[test]
    fn empty_names_fall_back_to_the_type_label() {
        let p = plan();
        let lc = p.find_by_name("Loop Controller").unwrap();
        assert_eq!(p.node(lc).unwrap().type_label, "Loop Controller");
    }

    #[test]
    fn parents_children_and_paths() {
        let p = plan();
        let get = p.find_by_name("GET /").unwrap();
        assert_eq!(p.depth(get), 3);
        assert_eq!(p.breadcrumb(get), ["Shop", "Users", "Loop Controller", "GET /"]);
        let users = p.find_by_name("Users").unwrap();
        assert_eq!(p.node(users).unwrap().children.len(), 2, "Loop Controller and Dummy; the listener is a sibling of Users");
        assert_eq!(p.descendant_count(users), 4);
        assert_eq!(p.node(get).unwrap().parent, p.find_by_name("Loop Controller"));
    }

    #[test]
    fn categories_flags_and_summary() {
        let p = plan();
        assert!(!p.node(p.find_by_name("ok").unwrap()).unwrap().enabled);
        let s = p.summary();
        assert_eq!(s.total, 7);
        assert_eq!(s.disabled, 1);
        assert_eq!(s.plugin, 1);
        assert_eq!(s.unknown, 0);
        let sampler_count = s.by_category.iter().find(|(c, _)| *c == Category::Sampler).unwrap().1;
        assert_eq!(sampler_count, 2); // HTTP sampler and the plugin Dummy Sampler
    }

    #[test]
    fn move_rules_and_cycles() {
        let p = plan();
        let users = p.find_by_name("Users").unwrap();
        let lc = p.find_by_name("Loop Controller").unwrap();
        let get = p.find_by_name("GET /").unwrap();
        let ok = p.find_by_name("ok").unwrap();
        let tree = p.find_by_name("Tree").unwrap();
        assert!(p.can_move(ok, lc), "an assertion can move under a controller");
        assert!(!p.can_move(get, ok), "nothing goes under an assertion");
        assert!(!p.can_move(lc, get), "a controller cannot go under a sampler");
        assert!(!p.can_move(users, lc), "a node cannot move into its own subtree");
        assert!(!p.can_move(lc, lc));
        assert!(p.can_move(tree, get), "a listener can sit under a sampler");
    }

    #[test]
    fn rejects_malformed_plans() {
        assert!(Plan::from_jmx_str("<notaplan/>").is_err());
        assert!(Plan::from_jmx_str("<jmeterTestPlan><hashTree><TestPlan/></hashTree></jmeterTestPlan>").is_err());
        assert!(Plan::from_jmx_str("<jmeterTestPlan>").is_err());
    }

    #[test]
    fn missing_file_error_names_the_path() {
        let e = Plan::load(Path::new("does/not/exist.jmx")).unwrap_err();
        assert!(format!("{e:#}").contains("exist.jmx"));
    }
}

impl Node {
    /// The value of a simple setting, if present.
    pub fn prop(&self, name: &str) -> Option<&str> {
        self.props.iter().find(|p| p.name == name).map(|p| p.value.as_str())
    }

    /// The element's comment, if it has one.
    pub fn comment(&self) -> Option<&str> {
        self.prop("TestPlan.comments").filter(|c| !c.is_empty())
    }
}

#[cfg(test)]
mod prop_tests {
    use super::*;

    #[test]
    fn props_and_comments_are_carried_over() {
        let p = Plan::from_jmx_str(
            r#"<jmeterTestPlan><hashTree>
<TestPlan testname="T"><stringProp name="TestPlan.comments">Smoke test</stringProp></TestPlan><hashTree>
 <HTTPSamplerProxy testname="S"><stringProp name="HTTPSampler.path">/api</stringProp><stringProp name="TestPlan.comments"></stringProp></HTTPSamplerProxy><hashTree/>
</hashTree></hashTree></jmeterTestPlan>"#,
        )
        .unwrap();
        let t = p.node(p.roots()[0]).unwrap();
        assert_eq!(t.comment(), Some("Smoke test"));
        let s = p.node(p.find_by_name("S").unwrap()).unwrap();
        assert_eq!(s.prop("HTTPSampler.path"), Some("/api"));
        assert_eq!(s.prop("nope"), None);
        assert_eq!(s.comment(), None, "an empty comment counts as none");
    }
}
