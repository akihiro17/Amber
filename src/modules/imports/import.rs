use std::fs;
use std::path::Path;
use heraclitus_compiler::prelude::*;
use crate::cli::cli_interface::CLI;
use crate::modules::block::Block;
use crate::utils::error::get_error_logger;
use crate::utils::exports::{Exports, ExportUnit};
use crate::utils::{ParserMetadata, TranslateMetadata};
use crate::translate::module::TranslateModule;
use super::import_string::ImportString;

#[derive(Debug, Clone)]
pub struct Import {
    path: ImportString,
    block: Block
}

impl Import {
    fn handle_export(&mut self, meta: &mut ParserMetadata, exports: Exports) {
        for export in exports.get_exports().iter().cloned() {
            match export {
                ExportUnit::Function(mut func_decl) => {
                    func_decl.is_public = false;
                    if !meta.mem.add_existing_function_declaration(func_decl) {
                        unimplemented!("Function redefinition");
                    }
                }
            }
        }
    }

    fn resolve_path(&mut self, meta: &mut ParserMetadata, tok: Option<Token>) -> String {
        let mut path = meta.path.as_ref()
            .map_or_else(|| Path::new("."), |path| Path::new(path))
            .to_path_buf();
        path.pop();
        path.push(&self.path.value);
        match path.to_str() {
            Some(path) => {
                if meta.import_history.add_import(meta.path.clone(), path.to_string()).is_none() {
                    let details = ErrorDetails::from_token_option(meta, tok);
                    get_error_logger(meta, details)
                        .attach_message("Circular import detected")
                        .attach_comment("Due to shell limitations, circular imports are not allowed")
                        .show()
                        .exit();
                }
                path.to_string()
            }
            None => {
                let details = ErrorDetails::from_token_option(meta, tok);
                get_error_logger(meta, details)
                    .attach_message(format!("Could not resolve path '{}'", path.display()))
                    .attach_comment("Path is not valid UTF-8")
                    .show()
                    .exit();
                String::new()
            }
        }
    }

    fn resolve_import(&mut self, meta: &mut ParserMetadata, tok: Option<Token>) -> String {
        match fs::read_to_string(self.resolve_path(meta, tok.clone())) {
            Ok(content) => content,
            Err(err) => {
                let details = ErrorDetails::from_token_option(meta, tok);
                get_error_logger(meta, details)
                    .attach_message(format!("Failed to read file '{}'", &self.path.value))
                    .attach_comment(err.to_string())
                    .show()
                    .exit();
                String::new()
            }
        }
    }

    fn handle_import(&mut self, meta: &mut ParserMetadata, tok: Option<Token>, imported_code: String) -> SyntaxResult {
        let cc = CLI::create_compiler(imported_code.clone());
        match cc.tokenize() {
            Ok(tokens) => {
                self.block.set_scopeless();
                // Save snapshot of current file
                let code = meta.code.clone();
                let path = meta.path.clone();
                let expr = meta.expr.clone();
                let exports = meta.mem.exports.clone();
                let index = meta.get_index();
                // Parse the imported file
                meta.push_trace(ErrorDetails::from_token_option(meta, tok));
                meta.path = Some(self.path.value.clone());
                meta.code = Some(imported_code);
                meta.expr = tokens;
                meta.set_index(0);
                syntax(meta, &mut self.block)?;
                self.handle_export(meta, meta.mem.exports.clone());
                // Restore snapshot of current file
                meta.code = code;
                meta.path = path;
                meta.expr = expr;
                meta.mem.exports = exports;
                meta.set_index(index);
                meta.pop_trace();
            },
            Err(error) => {
                CLI::tokenize_error(meta.path.clone(), imported_code, error);
            }
        }
        Ok(())
    }
}

impl SyntaxModule<ParserMetadata> for Import {
    syntax_name!("Import File");

    fn new() -> Self {
        Self {
            path: ImportString::new(),
            block: Block::new()
        }
    }

    fn parse(&mut self, meta: &mut ParserMetadata) -> SyntaxResult {
        token(meta, "import")?;
        let tok = meta.get_current_token();
        token(meta, "*")?;
        token(meta, "from")?;
        let tok_str = meta.get_current_token();
        syntax(meta, &mut self.path)?;
        let imported_code = self.resolve_import(meta, tok_str);
        self.handle_import(meta, tok, imported_code)?;
        Ok(())
    }
}

impl TranslateModule for Import {
    fn translate(&self, meta: &mut TranslateMetadata) -> String {
        self.block.translate(meta)
    }
}