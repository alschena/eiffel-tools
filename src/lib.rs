pub mod lib {
    mod code_entities;
    pub mod language_server_protocol {
        pub mod code_action;
        pub mod common;
        pub mod document_symbol;
        pub mod hover;
        pub mod initialization;
        pub mod workspace_document_symbol;
    }
    mod gemini;
    mod processed_file;
    mod tree_sitter;
    mod workspace;
}
