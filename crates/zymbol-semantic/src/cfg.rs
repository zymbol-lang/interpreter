//! Control Flow Graph (CFG) for Zymbol-Lang
//!
//! Provides CFG construction for lifetime analysis

use std::collections::HashMap;
use zymbol_ast::{Expr, IfStmt, Loop, MatchExpr, Statement, TryStmt};
use zymbol_span::Span;

/// Unique identifier for CFG nodes
pub type NodeId = usize;

/// A node in the control flow graph
#[derive(Debug, Clone)]
pub enum CfgNode {
    /// Entry point of the CFG
    Entry,
    /// Exit point of the CFG
    Exit,
    /// Regular statement execution
    Statement {
        stmt_index: usize,
        span: Span,
    },
    /// Conditional branch point
    Condition {
        expr: Box<Expr>,
        span: Span,
    },
}

/// Edge condition for control flow
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EdgeCondition {
    /// Unconditional edge (always taken)
    Always,
    /// Edge taken when condition is true
    True,
    /// Edge taken when condition is false
    False,
    /// Edge taken when exception is thrown
    Exception,
}

/// An edge in the control flow graph
#[derive(Debug, Clone)]
pub struct CfgEdge {
    /// Source node
    pub from: NodeId,
    /// Target node
    pub to: NodeId,
    /// Condition for taking this edge
    pub condition: EdgeCondition,
}

/// Control Flow Graph
#[derive(Debug, Clone)]
pub struct ControlFlowGraph {
    /// All nodes in the graph
    nodes: Vec<CfgNode>,
    /// All edges in the graph
    edges: Vec<CfgEdge>,
    /// Entry node ID
    pub entry: NodeId,
    /// Exit node ID
    pub exit: NodeId,
    /// Adjacency list: node -> list of outgoing edges
    successors: HashMap<NodeId, Vec<NodeId>>,
    /// Reverse adjacency list: node -> list of incoming edges
    predecessors: HashMap<NodeId, Vec<NodeId>>,
}

impl ControlFlowGraph {
    /// Create a new empty CFG
    pub fn new() -> Self {
        let mut cfg = Self {
            nodes: Vec::new(),
            edges: Vec::new(),
            entry: 0,
            exit: 0,
            successors: HashMap::new(),
            predecessors: HashMap::new(),
        };

        // Create entry and exit nodes
        cfg.entry = cfg.add_node(CfgNode::Entry);
        cfg.exit = cfg.add_node(CfgNode::Exit);

        cfg
    }

    /// Add a new node to the CFG
    fn add_node(&mut self, node: CfgNode) -> NodeId {
        let id = self.nodes.len();
        self.nodes.push(node);
        self.successors.insert(id, Vec::new());
        self.predecessors.insert(id, Vec::new());
        id
    }

    /// Add an edge between two nodes
    fn add_edge(&mut self, from: NodeId, to: NodeId, condition: EdgeCondition) {
        self.edges.push(CfgEdge {
            from,
            to,
            condition,
        });

        self.successors.entry(from).or_default().push(to);
        self.predecessors.entry(to).or_default().push(from);
    }

    /// Get all successor nodes of a given node
    pub fn successors(&self, node: NodeId) -> &[NodeId] {
        self.successors.get(&node).map(|v| v.as_slice()).unwrap_or(&[])
    }

    /// Get all predecessor nodes of a given node
    pub fn predecessors(&self, node: NodeId) -> &[NodeId] {
        self.predecessors.get(&node).map(|v| v.as_slice()).unwrap_or(&[])
    }

    /// Get a node by ID
    pub fn get_node(&self, id: NodeId) -> Option<&CfgNode> {
        self.nodes.get(id)
    }

    /// Build CFG for a block (Phase 1: sequential statements only)
    pub fn build_sequential(statements: &[Statement]) -> Self {
        let mut cfg = Self::new();

        if statements.is_empty() {
            // Empty block: just connect entry to exit
            cfg.add_edge(cfg.entry, cfg.exit, EdgeCondition::Always);
            return cfg;
        }

        let mut prev_node = cfg.entry;

        // Create a node for each statement
        for (index, stmt) in statements.iter().enumerate() {
            let node_id = cfg.add_node(CfgNode::Statement {
                stmt_index: index,
                span: *stmt.span(),
            });

            // Connect previous node to this one
            cfg.add_edge(prev_node, node_id, EdgeCondition::Always);
            prev_node = node_id;
        }

        // Connect last statement to exit
        cfg.add_edge(prev_node, cfg.exit, EdgeCondition::Always);

        cfg
    }

    /// Build complete CFG for a block (Phase 2: all control structures)
    pub fn build(statements: &[Statement]) -> Self {
        let mut cfg = Self::new();
        let entry = cfg.entry;
        let exit = cfg.exit;
        let mut builder = CfgBuilder::new(&mut cfg);
        builder.build_block(statements, entry, exit);
        cfg
    }

    /// Find all back edges in the CFG (for loop detection)
    /// Returns pairs of (tail, head) where tail -> head is a back edge
    pub fn find_back_edges(&self) -> Vec<(NodeId, NodeId)> {
        use std::collections::HashSet;

        let mut back_edges = Vec::new();
        let mut visited = HashSet::new();
        let mut in_stack = HashSet::new();

        fn dfs(
            node: NodeId,
            cfg: &ControlFlowGraph,
            visited: &mut HashSet<NodeId>,
            in_stack: &mut HashSet<NodeId>,
            back_edges: &mut Vec<(NodeId, NodeId)>,
        ) {
            visited.insert(node);
            in_stack.insert(node);

            for &successor in cfg.successors(node) {
                if !visited.contains(&successor) {
                    dfs(successor, cfg, visited, in_stack, back_edges);
                } else if in_stack.contains(&successor) {
                    // Back edge found (loop)
                    back_edges.push((node, successor));
                }
            }

            in_stack.remove(&node);
        }

        dfs(self.entry, self, &mut visited, &mut in_stack, &mut back_edges);
        back_edges
    }
}

/// Helper for building CFG with context (loop labels, etc.)
struct CfgBuilder<'a> {
    cfg: &'a mut ControlFlowGraph,
    /// Stack of loop contexts (for break/continue)
    /// Each entry is (loop_head, loop_exit, optional_label)
    loop_stack: Vec<(NodeId, NodeId, Option<String>)>,
    /// Statement index counter (for tracking statement positions)
    stmt_counter: usize,
}

impl<'a> CfgBuilder<'a> {
    fn new(cfg: &'a mut ControlFlowGraph) -> Self {
        Self {
            cfg,
            loop_stack: Vec::new(),
            stmt_counter: 0,
        }
    }

    /// Build CFG for a block of statements
    /// Returns the node where execution enters and exits
    fn build_block(
        &mut self,
        statements: &[Statement],
        entry: NodeId,
        exit: NodeId,
    ) -> (NodeId, NodeId) {
        if statements.is_empty() {
            self.cfg.add_edge(entry, exit, EdgeCondition::Always);
            return (entry, exit);
        }

        let mut current = entry;

        for stmt in statements {
            current = self.build_statement(stmt, current, exit);
        }

        // Connect final node to exit
        self.cfg.add_edge(current, exit, EdgeCondition::Always);

        (entry, exit)
    }

    /// Build CFG for a single statement
    /// Returns the node where execution continues after this statement
    fn build_statement(&mut self, stmt: &Statement, entry: NodeId, block_exit: NodeId) -> NodeId {
        match stmt {
            Statement::If(if_stmt) => self.build_if(if_stmt, entry, block_exit),
            Statement::Loop(loop_stmt) => self.build_loop(loop_stmt, entry, block_exit),
            Statement::Match(match_expr) => self.build_match(match_expr, entry, block_exit),
            Statement::Try(try_stmt) => self.build_try(try_stmt, entry, block_exit),
            Statement::Break(break_stmt) => self.build_break(&break_stmt.label, entry),
            Statement::Continue(continue_stmt) => self.build_continue(&continue_stmt.label, entry),
            Statement::Return(_) => {
                // Return jumps to function exit (for now, block exit)
                let stmt_node = self.create_statement_node(stmt);
                self.cfg.add_edge(entry, stmt_node, EdgeCondition::Always);
                self.cfg.add_edge(stmt_node, self.cfg.exit, EdgeCondition::Always);
                stmt_node // Dead code after return
            }
            _ => {
                // Regular statement: create node and connect
                let stmt_node = self.create_statement_node(stmt);
                self.cfg.add_edge(entry, stmt_node, EdgeCondition::Always);
                stmt_node
            }
        }
    }

    /// Create a statement node
    fn create_statement_node(&mut self, stmt: &Statement) -> NodeId {
        let index = self.stmt_counter;
        self.stmt_counter += 1;
        self.cfg.add_node(CfgNode::Statement {
            stmt_index: index,
            span: *stmt.span(),
        })
    }

    /// Build CFG for if/else-if/else statement
    fn build_if(&mut self, if_stmt: &IfStmt, entry: NodeId, _block_exit: NodeId) -> NodeId {
        // Create condition node
        let cond_node = self.cfg.add_node(CfgNode::Condition {
            expr: if_stmt.condition.clone(),
            span: if_stmt.span,
        });
        self.cfg.add_edge(entry, cond_node, EdgeCondition::Always);

        // Create merge node (where all branches converge)
        let merge_node = self.cfg.add_node(CfgNode::Statement {
            stmt_index: self.stmt_counter,
            span: if_stmt.span,
        });
        self.stmt_counter += 1;

        // Build then branch
        let then_entry = self.cfg.add_node(CfgNode::Statement {
            stmt_index: self.stmt_counter,
            span: if_stmt.then_block.span,
        });
        self.stmt_counter += 1;
        self.cfg.add_edge(cond_node, then_entry, EdgeCondition::True);
        self.build_block(&if_stmt.then_block.statements, then_entry, merge_node);

        // Handle else-if and else branches
        let mut current_cond = cond_node;

        for else_if in &if_stmt.else_if_branches {
            // Create else-if condition node
            let else_if_cond = self.cfg.add_node(CfgNode::Condition {
                expr: else_if.condition.clone(),
                span: else_if.span,
            });
            self.cfg.add_edge(current_cond, else_if_cond, EdgeCondition::False);

            // Build else-if block
            let else_if_entry = self.cfg.add_node(CfgNode::Statement {
                stmt_index: self.stmt_counter,
                span: else_if.block.span,
            });
            self.stmt_counter += 1;
            self.cfg.add_edge(else_if_cond, else_if_entry, EdgeCondition::True);
            self.build_block(&else_if.block.statements, else_if_entry, merge_node);

            current_cond = else_if_cond;
        }

        // Handle else branch or connect to merge
        if let Some(else_block) = &if_stmt.else_block {
            let else_entry = self.cfg.add_node(CfgNode::Statement {
                stmt_index: self.stmt_counter,
                span: else_block.span,
            });
            self.stmt_counter += 1;
            self.cfg.add_edge(current_cond, else_entry, EdgeCondition::False);
            self.build_block(&else_block.statements, else_entry, merge_node);
        } else {
            // No else: false branch goes directly to merge
            self.cfg.add_edge(current_cond, merge_node, EdgeCondition::False);
        }

        merge_node
    }

    /// Build CFG for loop statement
    fn build_loop(&mut self, loop_stmt: &Loop, entry: NodeId, _block_exit: NodeId) -> NodeId {
        // Create loop header (condition check or iteration)
        // For CFG purposes, we create a condition node even for infinite loops
        let loop_header = if let Some(cond) = &loop_stmt.condition {
            // While loop
            self.cfg.add_node(CfgNode::Condition {
                expr: cond.clone(),
                span: loop_stmt.span,
            })
        } else if let Some(iterable) = &loop_stmt.iterable {
            // For-each loop
            self.cfg.add_node(CfgNode::Condition {
                expr: iterable.clone(),
                span: loop_stmt.span,
            })
        } else {
            // Infinite loop - create a dummy true condition
            self.cfg.add_node(CfgNode::Statement {
                stmt_index: self.stmt_counter,
                span: loop_stmt.span,
            })
        };
        self.stmt_counter += 1;
        self.cfg.add_edge(entry, loop_header, EdgeCondition::Always);

        // Create loop body entry
        let body_entry = self.cfg.add_node(CfgNode::Statement {
            stmt_index: self.stmt_counter,
            span: loop_stmt.body.span,
        });
        self.stmt_counter += 1;

        // Create loop exit node
        let loop_exit = self.cfg.add_node(CfgNode::Statement {
            stmt_index: self.stmt_counter,
            span: loop_stmt.span,
        });
        self.stmt_counter += 1;

        // True edge: enter loop body
        self.cfg.add_edge(loop_header, body_entry, EdgeCondition::True);
        // False edge: exit loop
        self.cfg.add_edge(loop_header, loop_exit, EdgeCondition::False);

        // Push loop context for break/continue
        self.loop_stack.push((loop_header, loop_exit, loop_stmt.label.clone()));

        // Build loop body
        self.build_block(&loop_stmt.body.statements, body_entry, loop_header);

        // Pop loop context
        self.loop_stack.pop();

        // Back edge: body continues to header
        // (already connected by build_block)

        loop_exit
    }

    /// Build CFG for match statement
    fn build_match(&mut self, match_expr: &MatchExpr, entry: NodeId, _block_exit: NodeId) -> NodeId {
        // Create merge node where all cases converge
        let merge_node = self.cfg.add_node(CfgNode::Statement {
            stmt_index: self.stmt_counter,
            span: match_expr.span,
        });
        self.stmt_counter += 1;

        // For each case, create branch
        for case in &match_expr.cases {
            let case_entry = self.cfg.add_node(CfgNode::Statement {
                stmt_index: self.stmt_counter,
                span: case.span,
            });
            self.stmt_counter += 1;

            // Edge from entry to this case (simplified: always possible)
            self.cfg.add_edge(entry, case_entry, EdgeCondition::Always);

            // Build case body if it has one
            if let Some(block) = &case.block {
                self.build_block(&block.statements, case_entry, merge_node);
            } else {
                // No block: just jump to merge
                self.cfg.add_edge(case_entry, merge_node, EdgeCondition::Always);
            }
        }

        merge_node
    }

    /// Build CFG for try/catch/finally statement
    fn build_try(&mut self, try_stmt: &TryStmt, entry: NodeId, _block_exit: NodeId) -> NodeId {
        // Create merge node where all paths converge (after finally, or after catch if no finally)
        let merge_node = self.cfg.add_node(CfgNode::Statement {
            stmt_index: self.stmt_counter,
            span: try_stmt.span,
        });
        self.stmt_counter += 1;

        // Create finally entry node if present
        let finally_entry = if try_stmt.finally_clause.is_some() {
            let node = self.cfg.add_node(CfgNode::Statement {
                stmt_index: self.stmt_counter,
                span: try_stmt.span,
            });
            self.stmt_counter += 1;
            Some(node)
        } else {
            None
        };

        // The target for normal completion of try block
        let try_exit_target = finally_entry.unwrap_or(merge_node);

        // Create catch entry nodes for each catch clause
        let mut catch_entries: Vec<NodeId> = Vec::new();
        for catch in &try_stmt.catch_clauses {
            let catch_entry = self.cfg.add_node(CfgNode::Statement {
                stmt_index: self.stmt_counter,
                span: catch.span,
            });
            self.stmt_counter += 1;
            catch_entries.push(catch_entry);
        }

        // Build try block
        let try_body_entry = self.cfg.add_node(CfgNode::Statement {
            stmt_index: self.stmt_counter,
            span: try_stmt.try_block.span,
        });
        self.stmt_counter += 1;
        self.cfg.add_edge(entry, try_body_entry, EdgeCondition::Always);

        // For each statement in try block, add exception edges to catch clauses
        let mut current = try_body_entry;
        for stmt in &try_stmt.try_block.statements {
            let stmt_node = self.create_statement_node(stmt);
            self.cfg.add_edge(current, stmt_node, EdgeCondition::Always);

            // Add exception edges from this statement to all catch entries
            for &catch_entry in &catch_entries {
                self.cfg.add_edge(stmt_node, catch_entry, EdgeCondition::Exception);
            }

            current = stmt_node;
        }

        // Normal try block completion goes to finally or merge
        self.cfg.add_edge(current, try_exit_target, EdgeCondition::Always);

        // Build catch blocks
        for (i, catch) in try_stmt.catch_clauses.iter().enumerate() {
            let catch_entry = catch_entries[i];
            let catch_exit_target = finally_entry.unwrap_or(merge_node);

            let mut catch_current = catch_entry;
            for stmt in &catch.block.statements {
                let stmt_node = self.create_statement_node(stmt);
                self.cfg.add_edge(catch_current, stmt_node, EdgeCondition::Always);
                catch_current = stmt_node;
            }

            // Catch completion goes to finally or merge
            self.cfg.add_edge(catch_current, catch_exit_target, EdgeCondition::Always);
        }

        // Build finally block if present
        if let Some(ref finally) = try_stmt.finally_clause {
            let finally_entry_node = finally_entry.unwrap();
            let mut finally_current = finally_entry_node;

            for stmt in &finally.block.statements {
                let stmt_node = self.create_statement_node(stmt);
                self.cfg.add_edge(finally_current, stmt_node, EdgeCondition::Always);
                finally_current = stmt_node;
            }

            // Finally completion goes to merge
            self.cfg.add_edge(finally_current, merge_node, EdgeCondition::Always);
        }

        merge_node
    }

    /// Build CFG for break statement
    fn build_break(&mut self, label: &Option<String>, entry: NodeId) -> NodeId {
        // Find target loop
        let target = if let Some(label_name) = label {
            // Labeled break: find matching loop
            self.loop_stack
                .iter()
                .rev()
                .find(|(_, _, l)| l.as_ref() == Some(label_name))
                .map(|(_, exit, _)| *exit)
        } else {
            // Unlabeled break: innermost loop
            self.loop_stack.last().map(|(_, exit, _)| *exit)
        };

        if let Some(loop_exit) = target {
            // Create break node
            let break_node = self.cfg.add_node(CfgNode::Statement {
                stmt_index: self.stmt_counter,
                span: empty_span(),
            });
            self.stmt_counter += 1;
            self.cfg.add_edge(entry, break_node, EdgeCondition::Always);
            self.cfg.add_edge(break_node, loop_exit, EdgeCondition::Always);
            break_node
        } else {
            // No loop found (error case - should be caught by semantic analysis)
            entry
        }
    }

    /// Build CFG for continue statement
    fn build_continue(&mut self, label: &Option<String>, entry: NodeId) -> NodeId {
        // Find target loop
        let target = if let Some(label_name) = label {
            // Labeled continue: find matching loop
            self.loop_stack
                .iter()
                .rev()
                .find(|(_, _, l)| l.as_ref() == Some(label_name))
                .map(|(header, _, _)| *header)
        } else {
            // Unlabeled continue: innermost loop
            self.loop_stack.last().map(|(header, _, _)| *header)
        };

        if let Some(loop_header) = target {
            // Create continue node
            let continue_node = self.cfg.add_node(CfgNode::Statement {
                stmt_index: self.stmt_counter,
                span: empty_span(),
            });
            self.stmt_counter += 1;
            self.cfg.add_edge(entry, continue_node, EdgeCondition::Always);
            // Back edge to loop header
            self.cfg.add_edge(continue_node, loop_header, EdgeCondition::Always);
            continue_node
        } else {
            // No loop found (error case - should be caught by semantic analysis)
            entry
        }
    }
}

/// Helper to create an empty span
fn empty_span() -> Span {
    use zymbol_span::{FileId, Position};
    Span {
        file_id: FileId(0),
        start: Position { line: 0, column: 0, byte_offset: 0 },
        end: Position { line: 0, column: 0, byte_offset: 0 },
    }
}

impl Default for ControlFlowGraph {
    fn default() -> Self {
        Self::new()
    }
}

// Helper trait to get span from Statement
trait Spannable {
    fn span(&self) -> &Span;
}

impl Spannable for Statement {
    fn span(&self) -> &Span {
        match self {
            Statement::Output(s) => &s.span,
            Statement::Assignment(s) => &s.span,
            Statement::ConstDecl(s) => &s.span,
            Statement::LifetimeEnd(s) => &s.span,
            Statement::Input(s) => &s.span,
            Statement::If(s) => &s.span,
            Statement::Loop(s) => &s.span,
            Statement::Break(s) => &s.span,
            Statement::Continue(s) => &s.span,
            Statement::Try(s) => &s.span,
            Statement::Newline(s) => &s.span,
            Statement::FunctionDecl(s) => &s.span,
            Statement::Return(s) => &s.span,
            Statement::Match(s) => &s.span,
            Statement::Expr(s) => &s.span,
            Statement::CliArgsCapture(s) => &s.span,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use zymbol_ast::{Assignment, LiteralExpr};
    use zymbol_common::Literal;
    use zymbol_span::{FileId, Position};

    fn dummy_span() -> Span {
        Span {
            file_id: FileId(0),
            start: Position { line: 1, column: 1, byte_offset: 0 },
            end: Position { line: 1, column: 1, byte_offset: 0 },
        }
    }

    #[test]
    fn test_empty_cfg() {
        let cfg = ControlFlowGraph::build_sequential(&[]);

        assert_eq!(cfg.nodes.len(), 2); // Entry + Exit
        assert_eq!(cfg.edges.len(), 1); // Entry -> Exit
        assert_eq!(cfg.successors(cfg.entry).len(), 1);
        assert_eq!(cfg.successors(cfg.entry)[0], cfg.exit);
    }

    #[test]
    fn test_single_statement_cfg() {
        let stmt = Statement::Assignment(Assignment::new(
            "x".to_string(),
            Expr::Literal(LiteralExpr::new(Literal::Int(42), dummy_span())),
            dummy_span(),
        ));

        let cfg = ControlFlowGraph::build_sequential(&[stmt]);

        // Entry -> Statement -> Exit
        assert_eq!(cfg.nodes.len(), 3);
        assert_eq!(cfg.edges.len(), 2);

        let stmt_node = 2; // Entry=0, Exit=1, Statement=2
        assert_eq!(cfg.successors(cfg.entry), &[stmt_node]);
        assert_eq!(cfg.successors(stmt_node), &[cfg.exit]);
    }

    #[test]
    fn test_sequential_statements_cfg() {
        let stmts = vec![
            Statement::Assignment(Assignment::new(
                "x".to_string(),
                Expr::Literal(LiteralExpr::new(Literal::Int(1), dummy_span())),
                dummy_span(),
            )),
            Statement::Assignment(Assignment::new(
                "y".to_string(),
                Expr::Literal(LiteralExpr::new(Literal::Int(2), dummy_span())),
                dummy_span(),
            )),
            Statement::Assignment(Assignment::new(
                "z".to_string(),
                Expr::Literal(LiteralExpr::new(Literal::Int(3), dummy_span())),
                dummy_span(),
            )),
        ];

        let cfg = ControlFlowGraph::build_sequential(&stmts);

        // Entry -> S1 -> S2 -> S3 -> Exit
        assert_eq!(cfg.nodes.len(), 5);
        assert_eq!(cfg.edges.len(), 4);

        // Verify linear flow
        let mut current = cfg.entry;
        for i in 0..3 {
            let successors = cfg.successors(current);
            assert_eq!(successors.len(), 1);
            current = successors[0];

            // Verify it's a statement node
            match cfg.get_node(current) {
                Some(CfgNode::Statement { stmt_index, .. }) => {
                    assert_eq!(*stmt_index, i);
                }
                _ if current == cfg.exit => break,
                _ => panic!("Expected statement node"),
            }
        }

        // Last node should connect to exit
        assert_eq!(cfg.successors(current), &[cfg.exit]);
    }

    #[test]
    fn test_predecessors() {
        let stmts = vec![
            Statement::Assignment(Assignment::new(
                "x".to_string(),
                Expr::Literal(LiteralExpr::new(Literal::Int(1), dummy_span())),
                dummy_span(),
            )),
            Statement::Assignment(Assignment::new(
                "y".to_string(),
                Expr::Literal(LiteralExpr::new(Literal::Int(2), dummy_span())),
                dummy_span(),
            )),
        ];

        let cfg = ControlFlowGraph::build_sequential(&stmts);

        // Exit should have one predecessor (last statement)
        assert_eq!(cfg.predecessors(cfg.exit).len(), 1);

        // Entry should have no predecessors
        assert_eq!(cfg.predecessors(cfg.entry).len(), 0);
    }
}
