use crate::sql::table::Table;
use crate::storage_structure::relational::{
    Contract, RelationalAST, RelationalEntry,
};
use crate::storage_structure::typing::{ExprTy, SimpleExprTy};
use std::collections::HashMap;

pub type TableMap = HashMap<String, Table>;

pub struct TableBuilder {
    pub tables: TableMap,
}

impl TableBuilder {
    pub(crate) fn tables_from_contract(
        contract: &Contract,
    ) -> (Vec<Table>, Vec<String>, Vec<String>) {
        // Generate the SQL schema for this contract
        let mut builder = TableBuilder::new("storage");
        builder.populate(&contract.storage_ast);

        let nofunctions_tables = builder.get_functionless_table_prefixes();
        let mut noview_tables = nofunctions_tables.clone();
        noview_tables.push("entry.".to_string());
        let mut tables: Vec<Table> = builder.tables.into_values().collect();

        for (entrypoint, entrypoint_ast) in &contract.entrypoint_asts {
            let mut entrypoint_table_builder =
                TableBuilder::new(format!("entry.{}", entrypoint).as_str());
            entrypoint_table_builder.populate(entrypoint_ast);

            tables.append(
                &mut entrypoint_table_builder
                    .tables
                    .into_values()
                    .collect(),
            );
        }

        (tables, noview_tables, nofunctions_tables)
    }

    pub(crate) fn new(root_table_name: &str) -> Self {
        let mut res = Self {
            tables: TableMap::new(),
        };
        res.touch_table(root_table_name);
        res
    }

    fn get_functionless_table_prefixes(&self) -> Vec<String> {
        let mut res: Vec<String> = vec![];

        // All child tables of changes tables cannot have view definitions defined.
        // To get _ordered or _live rows for these child tables, simply join with id
        // of parent bigmap table (on which there are _live and _ordered views defined).
        res.extend(
            self.tables
                .values()
                .filter_map(|t| {
                    if t.contains_snapshots() {
                        None
                    } else {
                        Some(format!("{}.", t.name))
                    }
                })
                .collect::<Vec<String>>(),
        );
        res
    }

    fn add_column(&mut self, rel_entry: &RelationalEntry) {
        let mut table = self.get_table(&rel_entry.table_name);
        if rel_entry.is_index {
            table.add_index(&rel_entry.column_name, &rel_entry.column_type);
        } else {
            table.add_column(&rel_entry.column_name, &rel_entry.column_type);
        }
        self.store_table(table);
    }

    fn touch_table(&mut self, name: &str) {
        self.store_table(self.get_table(name))
    }

    fn get_table(&self, name: &str) -> Table {
        match self.tables.get(name) {
            Some(x) => x.clone(),
            None => {
                let mut t = Table::new(name.to_string());
                t.add_index(
                    "tx_context_id",
                    &ExprTy::SimpleExprTy(SimpleExprTy::Int),
                );
                t.add_column("id", &ExprTy::SimpleExprTy(SimpleExprTy::Int));
                t
            }
        }
    }

    fn store_table(&mut self, table: Table) {
        self.tables
            .insert(table.name.clone(), table);
    }

    pub(crate) fn populate(&mut self, rel_ast: &RelationalAST) {
        match rel_ast {
            RelationalAST::Pair {
                left_ast,
                right_ast,
            } => {
                self.populate(left_ast);
                self.populate(right_ast);
            }
            RelationalAST::Map {
                key_ast, value_ast, ..
            } => {
                self.populate(key_ast);
                self.populate(value_ast);
            }
            RelationalAST::BigMap {
                table,
                key_ast,
                value_ast,
                has_memory,
            } => {
                self.populate(key_ast);
                self.populate(value_ast);
                let mut t = self.get_table(table);

                t.add_index(
                    "bigmap_id",
                    &ExprTy::SimpleExprTy(SimpleExprTy::Int),
                );
                if *has_memory {
                    t.tracks_changes();

                    t.add_column(
                        "deleted",
                        &ExprTy::SimpleExprTy(SimpleExprTy::Bool),
                    );
                } else {
                    t.has_copy_pointers();
                }
                self.store_table(t);
            }
            RelationalAST::Option { elem_ast } => self.populate(elem_ast),
            RelationalAST::List {
                table,
                elems_unique,
                elems_ast,
            } => {
                self.populate(elems_ast);
                if !elems_unique {
                    let mut t = self.get_table(table);
                    t.no_uniqueness();
                    self.store_table(t);
                }
            }
            RelationalAST::OrEnumeration {
                or_unfold,
                left_table,
                left_ast,
                right_table,
                right_ast,
            } => {
                if let Some(or_unfold) = or_unfold {
                    self.add_column(or_unfold);
                    if or_unfold.is_index {
                        let mut t = self.get_table(&or_unfold.table_name);
                        t.no_uniqueness();
                        self.store_table(t);
                    }
                }

                if let Some(left_table) = left_table {
                    self.touch_table(left_table);
                    self.populate(left_ast);
                }
                if let Some(right_table) = right_table {
                    self.touch_table(right_table);
                    self.populate(right_ast);
                }
            }
            RelationalAST::Leaf { rel_entry } => self.add_column(rel_entry),
        }
    }
}
