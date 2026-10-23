//! The plan tree: JMX stores each element followed by a `<hashTree>` holding its children.

use crate::xml::{Document, Element};
use anyhow::{Result, bail};

#[derive(Debug, Clone)]
pub struct PlanNode {
    /// The element's `uid` (0 if the document was not indexed).
    pub uid: u32,
    /// XML tag: a short alias for core elements (`HTTPSamplerProxy`) or a full class name for plugins.
    pub tag: String,
    pub testclass: String,
    pub guiclass: String,
    pub testname: String,
    pub enabled: bool,
    /// Simple settings saved on the element, in file order.
    pub props: Vec<Prop>,
    pub children: Vec<PlanNode>,
}

impl PlanNode {
    /// Plugin and custom elements are saved with a full class name as their tag.
    pub fn is_plugin_class(&self) -> bool {
        self.tag.contains('.')
    }

    pub fn count(&self) -> usize {
        1 + self.children.iter().map(PlanNode::count).sum::<usize>()
    }

    pub fn visit(&self, f: &mut impl FnMut(&PlanNode)) {
        f(self);
        for c in &self.children {
            c.visit(f);
        }
    }
}

/// Build the plan tree from a parsed document. Returns the top-level nodes (Test Plan, WorkBench).
pub fn plan_tree(doc: &Document) -> Result<Vec<PlanNode>> {
    if doc.root.name != "jmeterTestPlan" {
        bail!("root element is <{}>, expected <jmeterTestPlan>", doc.root.name);
    }
    let Some(top) = doc.root.elements().find(|e| e.name == "hashTree") else {
        bail!("missing top-level <hashTree>");
    };
    read_level(top)
}

fn read_level(hash_tree: &Element) -> Result<Vec<PlanNode>> {
    let mut out = Vec::new();
    let mut iter = hash_tree.elements();
    while let Some(el) = iter.next() {
        if el.name == "hashTree" {
            bail!("<hashTree> without a preceding element");
        }
        let Some(next) = iter.next() else {
            bail!("element <{}> is not followed by <hashTree>", el.name);
        };
        if next.name != "hashTree" {
            bail!("element <{}> is followed by <{}>, expected <hashTree>", el.name, next.name);
        }
        out.push(PlanNode {
            uid: el.uid,
            tag: el.name.clone(),
            testclass: el.attr("testclass").unwrap_or_default(),
            guiclass: el.attr("guiclass").unwrap_or_default(),
            testname: el.attr("testname").unwrap_or_default(),
            enabled: el.attr("enabled").as_deref() != Some("false"),
            props: read_props(el),
            children: read_level(next)?,
        });
    }
    Ok(out)
}

/// One setting saved on an element, such as `HTTPSampler.path`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Prop {
    pub name: String,
    pub kind: PropKind,
    /// The value for simple settings; for collections, a short description such as `3 items`.
    pub value: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PropKind {
    String,
    Bool,
    Int,
    Long,
    Double,
    /// A list of values (`collectionProp`).
    Collection,
    /// A nested element (`elementProp`) or object (`objProp`).
    Nested,
}

/// Read the settings that sit directly on an element.
fn read_props(el: &Element) -> Vec<Prop> {
    el.elements()
        .filter_map(|c| {
            let kind = match c.name.as_str() {
                "stringProp" => PropKind::String,
                "boolProp" => PropKind::Bool,
                "intProp" => PropKind::Int,
                "longProp" => PropKind::Long,
                "doubleProp" => PropKind::Double,
                "collectionProp" => PropKind::Collection,
                "elementProp" | "objProp" => PropKind::Nested,
                _ => return None,
            };
            let name = c.attr("name").or_else(|| c.elements().find(|n| n.name == "name").map(|n| n.text())).unwrap_or_default();
            let value = match kind {
                PropKind::Collection | PropKind::Nested => {
                    let n = c.elements().count();
                    format!("{n} item{}", if n == 1 { "" } else { "s" })
                }
                _ => c.text(),
            };
            Some(Prop { name, kind, value })
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_simple_collection_and_nested_props() {
        let src = r#"<jmeterTestPlan><hashTree>
<TestPlan testname="T"><stringProp name="TestPlan.comments">a &amp; b</stringProp><boolProp name="TestPlan.functional_mode">false</boolProp>
<elementProp name="TestPlan.user_defined_variables" elementType="Arguments"><collectionProp name="Arguments.arguments"/></elementProp>
<collectionProp name="list"><stringProp name="1">x</stringProp><stringProp name="2">y</stringProp></collectionProp>
<intProp name="n">5</intProp><somethingElse/></TestPlan><hashTree/>
</hashTree></jmeterTestPlan>"#;
        let doc = Document::parse(src).unwrap();
        let nodes = plan_tree(&doc).unwrap();
        let props = &nodes[0].props;
        assert_eq!(props.len(), 5, "unrelated children are skipped");
        assert_eq!(props[0], Prop { name: "TestPlan.comments".into(), kind: PropKind::String, value: "a & b".into() });
        assert_eq!(props[1].kind, PropKind::Bool);
        assert_eq!(props[1].value, "false");
        assert_eq!(props[2].kind, PropKind::Nested);
        assert_eq!(props[2].value, "1 item");
        assert_eq!(props[3].value, "2 items");
        assert_eq!(props[4].kind, PropKind::Int);
    }
}
