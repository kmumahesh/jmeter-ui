//! Which elements may be added under which.
//!
//! Based on `MenuFactory.canAddTo` in the JMeter source, widened where real plans do more than that
//! check allows: the WorkBench holds recorder elements, and the recorder holds timers, listeners and
//! other processors. The widening comes from running every sample plan through these rules.

use crate::catalog::Category::{self, *};

/// Can an element of category `child` be added under an element of category `parent`?
///
/// - A Test Plan cannot be added anywhere.
/// - Thread Groups and Test Fragments only go directly under the Test Plan.
/// - Non-test elements (recorder, mirror server) go under the Test Plan, a Test Fragment or the WorkBench.
/// - The Test Plan accepts everything else except samplers and controllers.
/// - Thread Groups, Logic Controllers and Test Fragments accept everything else.
/// - The WorkBench accepts everything else.
/// - Samplers accept everything except samplers and controllers.
/// - Non-test elements such as the recorder accept config elements, timers, assertions, processors and listeners.
/// - Config elements, timers, assertions, processors and listeners accept nothing.
/// - Unknown (plugin) elements are treated permissively so they are never locked out.
pub fn can_add(parent: Category, child: Category) -> bool {
    match child {
        TestPlan => return false,
        Fragment | ThreadGroup => return parent == TestPlan,
        NonTest => return matches!(parent, TestPlan | Fragment | WorkBench),
        WorkBench => return false,
        _ => {}
    }
    match parent {
        TestPlan => !matches!(child, Sampler | Controller),
        ThreadGroup | Controller | Fragment | WorkBench | Unknown => true,
        Sampler => !matches!(child, Sampler | Controller),
        NonTest => matches!(child, Config | Timer | PreProcessor | PostProcessor | Assertion | Listener),
        _ => false,
    }
}

/// Categories that may be added under `parent`, in menu order.
pub fn allowed_children(parent: Category) -> Vec<Category> {
    Category::ALL.iter().copied().filter(|&c| c != Unknown && can_add(parent, c)).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_plan_and_workbench_are_never_children() {
        for p in Category::ALL {
            assert!(!can_add(p, TestPlan), "{p:?}");
            assert!(!can_add(p, WorkBench), "{p:?}");
        }
    }

    #[test]
    fn thread_groups_and_fragments_only_under_the_plan() {
        for p in Category::ALL {
            assert_eq!(can_add(p, ThreadGroup), p == TestPlan, "thread group under {p:?}");
            assert_eq!(can_add(p, Fragment), p == TestPlan, "fragment under {p:?}");
        }
    }

    #[test]
    fn non_test_elements_go_under_plan_fragment_or_workbench() {
        for p in Category::ALL {
            assert_eq!(can_add(p, NonTest), matches!(p, TestPlan | Fragment | WorkBench), "{p:?}");
        }
    }

    #[test]
    fn plan_accepts_config_timers_listeners_but_not_samplers_or_controllers() {
        for c in [Config, Timer, PreProcessor, PostProcessor, Assertion, Listener, ThreadGroup, NonTest, Fragment] {
            assert!(can_add(TestPlan, c), "{c:?}");
        }
        for c in [Sampler, Controller] {
            assert!(!can_add(TestPlan, c), "{c:?}");
        }
    }

    #[test]
    fn controllers_and_thread_groups_accept_samplers_and_controllers() {
        for p in [ThreadGroup, Controller, Fragment] {
            for c in [Sampler, Controller, Config, Timer, PreProcessor, PostProcessor, Assertion, Listener] {
                assert!(can_add(p, c), "{c:?} under {p:?}");
            }
        }
    }

    #[test]
    fn workbench_accepts_everything_except_thread_groups_and_structure() {
        for c in [Sampler, Controller, Config, Timer, PreProcessor, PostProcessor, Assertion, Listener, NonTest] {
            assert!(can_add(WorkBench, c), "{c:?}");
        }
        assert!(!can_add(WorkBench, ThreadGroup));
        assert!(!can_add(WorkBench, Fragment));
    }

    #[test]
    fn samplers_accept_processors_but_not_samplers_or_controllers() {
        for c in [Config, Timer, PreProcessor, PostProcessor, Assertion, Listener] {
            assert!(can_add(Sampler, c), "{c:?}");
        }
        assert!(!can_add(Sampler, Sampler));
        assert!(!can_add(Sampler, Controller));
    }

    #[test]
    fn recorder_accepts_processors_but_not_samplers() {
        for c in [Config, Timer, PreProcessor, PostProcessor, Assertion, Listener] {
            assert!(can_add(NonTest, c), "{c:?}");
        }
        for c in [Sampler, Controller, ThreadGroup, NonTest] {
            assert!(!can_add(NonTest, c), "{c:?}");
        }
    }

    #[test]
    fn leaf_categories_accept_nothing() {
        for p in [Config, Timer, PreProcessor, PostProcessor, Assertion, Listener] {
            for c in Category::ALL {
                assert!(!can_add(p, c), "{c:?} under {p:?}");
            }
        }
    }

    #[test]
    fn unknown_parent_is_permissive_but_still_blocks_structural_children() {
        assert!(can_add(Unknown, Sampler));
        assert!(can_add(Unknown, Assertion));
        assert!(!can_add(Unknown, ThreadGroup));
        assert!(!can_add(Unknown, TestPlan));
    }

    #[test]
    fn allowed_children_lists() {
        let under_sampler = allowed_children(Sampler);
        assert_eq!(under_sampler, vec![Config, Timer, PreProcessor, PostProcessor, Assertion, Listener]);
        assert!(allowed_children(Listener).is_empty());
        let under_plan = allowed_children(TestPlan);
        assert!(under_plan.contains(&ThreadGroup) && !under_plan.contains(&Sampler));
    }
}
