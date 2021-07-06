use super::DatamodelError;
use crate::diagnostics::warning::DatamodelWarning;

/// Represents a list of validation or parser errors and warnings.
///
/// This is used to accumulate multiple errors and warnings during validation.
/// It is used to not error out early and instead show multiple errors at once.
#[derive(Debug, Clone)]
pub struct Diagnostics {
    /// TODO: these two fields are public for tests. We should strive towards
    /// making them private.
    pub errors: Vec<DatamodelError>,
    pub warnings: Vec<DatamodelWarning>,
}

impl Diagnostics {
    pub(crate) fn new() -> Diagnostics {
        Diagnostics {
            errors: Vec::new(),
            warnings: Vec::new(),
        }
    }

    pub(crate) fn push_error(&mut self, err: DatamodelError) {
        self.errors.push(err)
    }

    pub(crate) fn push_warning(&mut self, warning: DatamodelWarning) {
        self.warnings.push(warning)
    }

    /// Returns true, if there is at least one error in this collection.
    pub fn has_errors(&self) -> bool {
        !self.errors.is_empty()
    }

    pub fn has_warnings(&self) -> bool {
        !self.warnings.is_empty()
    }

    /// Creates an iterator over all errors in this collection.
    pub fn to_error_iter(&self) -> std::slice::Iter<'_, DatamodelError> {
        self.errors.iter()
    }

    /// Creates an iterator over all warnings in this collection.
    pub fn to_warning_iter(&self) -> std::slice::Iter<'_, DatamodelWarning> {
        self.warnings.iter()
    }

    /// Appends all errors and warnings from another collection to this collection.
    pub(crate) fn append(&mut self, err_and_warn: &mut Diagnostics) {
        self.errors.append(&mut err_and_warn.errors);
        self.warnings.append(&mut err_and_warn.warnings)
    }

    pub(crate) fn append_warning_vec(&mut self, mut warnings: Vec<DatamodelWarning>) {
        self.warnings.append(&mut warnings);
    }

    pub(crate) fn to_result(&self) -> Result<(), Diagnostics> {
        if self.has_errors() {
            Err(self.clone())
        } else {
            Ok(())
        }
    }

    pub fn to_pretty_string(&self, file_name: &str, datamodel_string: &str) -> String {
        let mut message: Vec<u8> = Vec::new();

        for err in self.to_error_iter() {
            err.pretty_print(&mut message, file_name, datamodel_string)
                .expect("printing datamodel error");
        }

        String::from_utf8_lossy(&message).into_owned()
    }
}

impl std::fmt::Display for Diagnostics {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let msg: Vec<String> = self.errors.iter().map(|e| e.to_string()).collect();
        f.write_str(&msg.join("\n"))
    }
}

impl From<DatamodelError> for Diagnostics {
    fn from(error: DatamodelError) -> Self {
        let mut col = Diagnostics::new();
        col.push_error(error);
        col
    }
}

impl From<DatamodelWarning> for Diagnostics {
    fn from(warning: DatamodelWarning) -> Self {
        let mut col = Diagnostics::new();
        col.push_warning(warning);
        col
    }
}

impl Default for Diagnostics {
    fn default() -> Self {
        Self::new()
    }
}
