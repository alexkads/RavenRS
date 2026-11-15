//! Running a diagram as a process, a step at a time: tokens leave the starts, go along the
//! connections, wait at joins and stop at the ends, and what goes wrong is said where it
//! happens. It knows no notation: rules say how each node lets a token through.

use std::rc::Rc;

use super::{FlowEdge, FlowGraph, FlowNode};

/// How a node lets a token through.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum FlowGate {
    /// A token starts here when the run starts, and goes on along every way out.
    Start,
    /// It goes on along every way out.
    Pass,
    /// It goes on along one way out, chosen when there is more than one.
    Choose,
    /// It waits for a token from every way in, then goes on along every way out.
    Parallel,
    /// It stops here.
    End,
    /// No token comes here (notes, data).
    Outside,
}

/// What a run needs to know of a notation.
#[derive(Clone)]
pub struct FlowRunRules {
    /// How each node lets a token through.
    pub gate: Rc<dyn Fn(&FlowNode) -> FlowGate>,
    /// Which connections a token follows; the others (associations, messages) it does not.
    pub follows: Rc<dyn Fn(&FlowEdge) -> bool>,
}

impl std::fmt::Debug for FlowRunRules {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("FlowRunRules").finish_non_exhaustive()
    }
}

/// Something that happened in a run, by the ids it is about.
#[derive(Clone, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum FlowRunEvent {
    /// A token left `from` along `edge` for `to`.
    Moved {
        /// Where it was.
        from: String,
        /// Where it went.
        to: String,
        /// The connection it took.
        edge: String,
    },
    /// A token reached an end.
    Ended {
        /// The end.
        node: String,
    },
    /// A token waits at a node for a way out to be chosen.
    Choosing {
        /// The node.
        node: String,
    },
    /// Every token reached an end.
    Finished,
    /// A token reached a node that leads nowhere and is not an end.
    DeadEnd {
        /// The node.
        node: String,
    },
    /// Tokens wait at a join for ways that never come.
    Stuck {
        /// The join.
        node: String,
    },
    /// The run went round without ending.
    Looping,
    /// The diagram has nowhere to start from.
    NoStart,
}

impl FlowRunEvent {
    /// Whether it says something went wrong.
    pub fn is_error(&self) -> bool {
        matches!(self, Self::DeadEnd { .. } | Self::Stuck { .. } | Self::Looping | Self::NoStart)
    }

    /// The node it is about, if one.
    pub fn node(&self) -> Option<&str> {
        match self {
            Self::Moved { to, .. } => Some(to),
            Self::Ended { node }
            | Self::Choosing { node }
            | Self::DeadEnd { node }
            | Self::Stuck { node } => Some(node),
            _ => None,
        }
    }
}

/// The most steps a run takes before it is said to go round.
pub const RUN_LIMIT: usize = 500;

/// A run of a diagram: where its tokens are, where they have been and what happened.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct FlowRun {
    /// The nodes holding a token, the next to move first.
    pub tokens: Vec<String>,
    /// The nodes tokens have left or ended at, in order.
    pub visited: Vec<String>,
    /// The connections tokens have taken, in order.
    pub taken: Vec<String>,
    /// Tokens waiting at a join: the join and the connection each came along.
    pub waiting: Vec<(String, String)>,
    /// The node whose way out is to be chosen before the run goes on.
    pub choosing: Option<String>,
    /// What happened, in order.
    pub events: Vec<FlowRunEvent>,
    steps: usize,
}

impl FlowRun {
    /// A run with a token at every start of `graph`.
    pub fn start(graph: &FlowGraph, rules: &FlowRunRules) -> Self {
        let tokens: Vec<String> =
            graph.nodes.iter().filter(|n| (rules.gate)(n) == FlowGate::Start).map(|n| n.id.clone()).collect();
        let events = if tokens.is_empty() { vec![FlowRunEvent::NoStart] } else { Vec::new() };
        Self { tokens, events, ..Self::default() }
    }

    /// Whether it ended, well or not.
    pub fn is_over(&self) -> bool {
        self.events.last().is_some_and(|e| *e == FlowRunEvent::Finished || e.is_error())
    }

    /// What went wrong, if it did.
    pub fn failure(&self) -> Option<&FlowRunEvent> {
        self.events.last().filter(|e| e.is_error())
    }

    /// The ways out of `node` a token can take.
    pub fn ways_out<'a>(graph: &'a FlowGraph, rules: &FlowRunRules, node: &str) -> Vec<&'a FlowEdge> {
        graph.edges.iter().filter(|e| e.from.node == node && (rules.follows)(e)).collect()
    }

    /// Moves the next token one node on. While a way out is to be chosen, `choice` (a
    /// connection's id) chooses it; without it, nothing moves.
    pub fn step(&mut self, graph: &FlowGraph, rules: &FlowRunRules, choice: Option<&str>) {
        if self.is_over() {
            return;
        }
        self.steps += 1;
        if self.steps > RUN_LIMIT {
            self.events.push(FlowRunEvent::Looping);
            return;
        }
        if let Some(node) = self.choosing.clone() {
            let ways = Self::ways_out(graph, rules, &node);
            let Some(edge) = choice.and_then(|c| ways.iter().find(|e| e.id == c)).copied() else {
                self.steps -= 1;
                return;
            };
            self.choosing = None;
            if let Some(i) = self.tokens.iter().position(|t| *t == node) {
                self.tokens.remove(i);
            }
            self.visited.push(node.clone());
            let edge = edge.clone();
            self.advance(graph, rules, &node, &edge);
            self.settle();
            return;
        }
        let Some(node) = self.tokens.first().cloned() else {
            self.settle();
            return;
        };
        let Some(at) = graph.find_node(&node) else {
            self.tokens.remove(0);
            return;
        };
        let gate = (rules.gate)(at);
        let ways: Vec<FlowEdge> = Self::ways_out(graph, rules, &node).into_iter().cloned().collect();
        match gate {
            FlowGate::End => {
                self.tokens.remove(0);
                self.visited.push(node.clone());
                self.events.push(FlowRunEvent::Ended { node });
            }
            _ if ways.is_empty() => {
                self.tokens.remove(0);
                self.events.push(FlowRunEvent::DeadEnd { node });
                return;
            }
            FlowGate::Choose if ways.len() > 1 => {
                self.choosing = Some(node.clone());
                self.events.push(FlowRunEvent::Choosing { node });
                return;
            }
            FlowGate::Choose => {
                self.tokens.remove(0);
                self.visited.push(node.clone());
                self.advance(graph, rules, &node, &ways[0]);
            }
            _ => {
                self.tokens.remove(0);
                self.visited.push(node.clone());
                for way in &ways {
                    self.advance(graph, rules, &node, way);
                }
            }
        }
        self.settle();
    }

    /// A token from `from` along `edge`: it reaches the node, or waits there for the others.
    fn advance(&mut self, graph: &FlowGraph, rules: &FlowRunRules, from: &str, edge: &FlowEdge) {
        let to = edge.to.node.clone();
        self.taken.push(edge.id.clone());
        self.events.push(FlowRunEvent::Moved {
            from: from.to_owned(),
            to: to.clone(),
            edge: edge.id.clone(),
        });
        let join = graph.find_node(&to).is_some_and(|n| (rules.gate)(n) == FlowGate::Parallel);
        let ins: Vec<&FlowEdge> =
            graph.edges.iter().filter(|e| e.to.node == to && (rules.follows)(e)).collect();
        if join && ins.len() > 1 {
            self.waiting.push((to.clone(), edge.id.clone()));
            let came: Vec<&str> =
                self.waiting.iter().filter(|(j, _)| *j == to).map(|(_, e)| e.as_str()).collect();
            if ins.iter().all(|e| came.contains(&e.id.as_str())) {
                self.waiting.retain(|(j, _)| *j != to);
                self.tokens.push(to);
            }
        } else {
            self.tokens.push(to);
        }
    }

    /// With no token left to move, the run is over: well, or stuck at a join.
    fn settle(&mut self) {
        if !self.tokens.is_empty() || self.choosing.is_some() {
            return;
        }
        match self.waiting.first() {
            Some((join, _)) => self.events.push(FlowRunEvent::Stuck { node: join.clone() }),
            None => self.events.push(FlowRunEvent::Finished),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::flow::{FlowEndpoint, FlowPort};

    fn node(id: &str, kind: &str) -> FlowNode {
        FlowNode::new(id, kind, id, 0.0, 0.0).port(FlowPort::input("in")).port(FlowPort::output("out"))
    }

    fn edge(id: &str, from: &str, to: &str) -> FlowEdge {
        FlowEdge::new(id, FlowEndpoint::new(from, "out"), FlowEndpoint::new(to, "in"))
    }

    fn rules() -> FlowRunRules {
        FlowRunRules {
            gate: Rc::new(|n: &FlowNode| match n.kind.as_str() {
                "start" => FlowGate::Start,
                "end" => FlowGate::End,
                "choice" => FlowGate::Choose,
                "fork" => FlowGate::Parallel,
                "note" => FlowGate::Outside,
                _ => FlowGate::Pass,
            }),
            follows: Rc::new(|e: &FlowEdge| e.kind.is_empty()),
        }
    }

    fn run_out(graph: &FlowGraph, choices: &[&str]) -> FlowRun {
        let rules = rules();
        let mut run = FlowRun::start(graph, &rules);
        let mut choices = choices.iter();
        while !run.is_over() {
            let choice = run.choosing.as_ref().and_then(|_| choices.next().copied());
            let before = run.clone();
            run.step(graph, &rules, choice);
            if run == before {
                break;
            }
        }
        run
    }

    #[test]
    fn a_token_goes_from_the_start_to_the_end_by_the_way_chosen() {
        let g = FlowGraph::new()
            .node(node("s", "start"))
            .node(node("c", "choice"))
            .node(node("a", "task"))
            .node(node("b", "task"))
            .node(node("e", "end"))
            .edge(edge("1", "s", "c"))
            .edge(edge("2", "c", "a"))
            .edge(edge("3", "c", "b"))
            .edge(edge("4", "a", "e"))
            .edge(edge("5", "b", "e"));
        let rules = rules();
        let mut run = FlowRun::start(&g, &rules);
        run.step(&g, &rules, None);
        run.step(&g, &rules, None);
        assert_eq!(run.choosing.as_deref(), Some("c"), "it waits for a choice");
        run.step(&g, &rules, None);
        assert_eq!(run.choosing.as_deref(), Some("c"), "and nothing moves without one");
        run.step(&g, &rules, Some("3"));
        let done = run_out(&g, &["3"]);
        assert_eq!(done.events.last(), Some(&FlowRunEvent::Finished));
        assert_eq!(done.taken, ["1", "3", "5"]);
        assert!(!done.visited.contains(&"a".to_owned()));
    }

    #[test]
    fn a_parallel_join_waits_for_every_way_and_says_when_one_never_comes() {
        let both = FlowGraph::new()
            .node(node("s", "start"))
            .node(node("f", "fork"))
            .node(node("a", "task"))
            .node(node("b", "task"))
            .node(node("j", "fork"))
            .node(node("e", "end"))
            .edge(edge("1", "s", "f"))
            .edge(edge("2", "f", "a"))
            .edge(edge("3", "f", "b"))
            .edge(edge("4", "a", "j"))
            .edge(edge("5", "b", "j"))
            .edge(edge("6", "j", "e"));
        let done = run_out(&both, &[]);
        assert_eq!(done.events.last(), Some(&FlowRunEvent::Finished));
        assert_eq!(done.taken.iter().filter(|t| *t == "6").count(), 1, "the join goes on once");

        // A choice before the join sends only one way: the join is stuck.
        let mut one = both.clone();
        one.nodes[1].kind = "choice".into();
        let stuck = run_out(&one, &["2"]);
        assert_eq!(stuck.failure(), Some(&FlowRunEvent::Stuck { node: "j".into() }));
    }

    #[test]
    fn dead_ends_loops_and_no_start_are_said() {
        let dead =
            FlowGraph::new().node(node("s", "start")).node(node("a", "task")).edge(edge("1", "s", "a"));
        assert_eq!(run_out(&dead, &[]).failure(), Some(&FlowRunEvent::DeadEnd { node: "a".into() }));
        let round = FlowGraph::new()
            .node(node("s", "start"))
            .node(node("a", "task"))
            .node(node("b", "task"))
            .edge(edge("1", "s", "a"))
            .edge(edge("2", "a", "b"))
            .edge(edge("3", "b", "a"));
        assert_eq!(run_out(&round, &[]).failure(), Some(&FlowRunEvent::Looping));
        let none = FlowGraph::new().node(node("a", "task"));
        assert_eq!(FlowRun::start(&none, &rules()).failure(), Some(&FlowRunEvent::NoStart));
        // Notes and associations take no token.
        let noted = dead
            .clone()
            .node(node("n", "note"))
            .edge(FlowEdge { kind: "association".into(), ..edge("x", "a", "n") });
        assert_eq!(run_out(&noted, &[]).failure(), Some(&FlowRunEvent::DeadEnd { node: "a".into() }));
    }
}
