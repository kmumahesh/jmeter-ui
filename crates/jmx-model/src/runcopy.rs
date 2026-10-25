//! Edits applied to a per-run copy of a plan. The user's file is never changed.

use crate::xml::{Document, Element, Node, escape, parse_fragment};
use anyhow::{Context, Result, bail};

/// Which samples a collector writes and in what shape.
pub enum Collector<'a> {
    /// Failed samples only, as XML including request and response data.
    XmlErrors { path: &'a str },
}

/// Add a results collector as a child of the Test Plan, so it sees every sampler.
pub fn inject_collector(doc: &mut Document, collector: &Collector) -> Result<()> {
    let Collector::XmlErrors { path } = collector;
    let template = format!(
        r#"<ResultCollector guiclass="SimpleDataWriter" testclass="ResultCollector" testname="Run errors (xml)" enabled="true">
  <boolProp name="ResultCollector.error_logging">true</boolProp>
  <objProp>
    <name>saveConfig</name>
    <value class="SampleSaveConfiguration">
      <time>true</time><latency>true</latency><timestamp>true</timestamp><success>true</success>
      <label>true</label><code>true</code><message>true</message><threadName>true</threadName>
      <dataType>true</dataType><encoding>false</encoding><assertions>true</assertions>
      <subresults>true</subresults><responseData>true</responseData><samplerData>true</samplerData>
      <xml>true</xml><fieldNames>true</fieldNames><responseHeaders>true</responseHeaders>
      <requestHeaders>true</requestHeaders><responseDataOnError>false</responseDataOnError>
      <saveAssertionResultsFailureMessage>true</saveAssertionResultsFailureMessage>
      <assertionsResultsToSave>0</assertionsResultsToSave><bytes>true</bytes>
      <threadCounts>true</threadCounts><sampleCount>true</sampleCount><url>true</url>
    </value>
  </objProp>
  <stringProp name="filename">{}</stringProp>
</ResultCollector>"#,
        escape(path, false)
    );
    insert_under_plan(doc, &template)
}

/// Add a JSR223 Listener (Groovy) with `script` as the last child of the Test Plan, so it sees every sampler. The
/// plan JMeter runs gets it; the user's file does not.
pub fn inject_script_listener(doc: &mut Document, name: &str, script: &str) -> Result<()> {
    let template = format!(
        r#"<JSR223Listener guiclass="TestBeanGUI" testclass="JSR223Listener" testname="{}" enabled="true">
  <stringProp name="scriptLanguage">groovy</stringProp>
  <stringProp name="parameters"></stringProp>
  <stringProp name="filename"></stringProp>
  <stringProp name="cacheKey">true</stringProp>
  <stringProp name="script">{}</stringProp>
</JSR223Listener>"#,
        escape(name, true),
        escape(script, false)
    );
    insert_under_plan(doc, &template)
}

/// Put an element (and an empty `<hashTree/>` after it) last under the Test Plan.
fn insert_under_plan(doc: &mut Document, element_xml: &str) -> Result<()> {
    let collector_el = parse_fragment(element_xml)?;
    let tree_el = parse_fragment("<hashTree/>")?;

    let top = doc
        .root
        .children
        .iter_mut()
        .find_map(|n| match n {
            Node::Element(e) if e.name == "hashTree" => Some(e),
            _ => None,
        })
        .context("missing top-level <hashTree>")?;
    // The Test Plan's children are in the hashTree that follows the TestPlan element.
    let mut seen_plan = false;
    for n in &mut top.children {
        if let Node::Element(e) = n {
            if e.name == "TestPlan" {
                seen_plan = true;
            } else if seen_plan && e.name == "hashTree" {
                e.push(collector_el);
                e.push(tree_el);
                return Ok(());
            }
        }
    }
    bail!("no TestPlan found")
}

/// Set every `LoopController.loops` to `loops` (`-1` means forever). Returns how many were changed.
pub fn set_loops(doc: &mut Document, loops: i32) -> usize {
    let mut n = 0;
    doc.root.walk_mut(&mut |e: &mut Element| {
        if e.name == "stringProp" && e.attr("name").as_deref() == Some("LoopController.loops") {
            e.set_text(&loops.to_string());
            n += 1;
        }
    });
    n
}
