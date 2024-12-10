use crate::lib::code_entities::prelude::*;
use async_lsp::lsp_types::TextEdit;
use contract::{Postcondition, Precondition};

pub(crate) fn text_edit_add_postcondition(
    feature: &Feature,
    point: Point,
    postcondition: Postcondition,
) -> TextEdit {
    let postcondition_text = if feature.has_postcondition() {
        format!("{postcondition}")
    } else {
        format!(
            "{}",
            contract::Block::<contract::Postcondition>::new(
                postcondition,
                Range::new_collapsed(point.clone())
            )
        )
    };
    TextEdit {
        range: Range::new_collapsed(point)
            .try_into()
            .expect("range should convert to lsp-type range."),
        new_text: postcondition_text,
    }
}
pub(crate) fn text_edit_add_precondition(
    feature: &Feature,
    point: Point,
    precondition: Precondition,
) -> TextEdit {
    let precondition_text = if feature.has_precondition() {
        format!("{precondition}")
    } else {
        format!(
            "{}",
            contract::Block::<contract::Precondition>::new(
                precondition,
                Range::new_collapsed(point.clone())
            )
        )
    };
    TextEdit {
        range: Range::new_collapsed(point)
            .try_into()
            .expect("range should convert to lsp-type range."),
        new_text: precondition_text,
    }
}
