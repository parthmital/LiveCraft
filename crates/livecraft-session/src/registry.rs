//! The command registry: stable names mapped to constructors that decode JSON arguments (M04 item 2).
//!
//! The CLI and the control server (M42) run any command by its name with JSON arguments. Each
//! command module registers its operations, so a new command is one registration.

use crate::command::{Command, CommandError, Operation};
use std::collections::BTreeMap;

type Build = fn(serde_json::Value) -> Result<Box<dyn Command>, CommandError>;

#[derive(Default)]
pub struct CommandRegistry {
    builders: BTreeMap<&'static str, Build>,
}

impl CommandRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    /// Registers an operation under its `NAME`. A name is registered once; a second registration
    /// of the same name is a programming error.
    pub fn register<O: Operation>(&mut self) {
        let previous = self.builders.insert(O::NAME, build::<O>);
        debug_assert!(
            previous.is_none(),
            "command `{}` is registered twice",
            O::NAME
        );
    }

    /// Every registered name, in order.
    pub fn names(&self) -> impl Iterator<Item = &'static str> + '_ {
        self.builders.keys().copied()
    }

    /// Builds a command from its name and JSON arguments. An unknown name or arguments that do not
    /// decode are errors.
    pub fn build(
        &self,
        name: &str,
        args: serde_json::Value,
    ) -> Result<Box<dyn Command>, CommandError> {
        let build = self
            .builders
            .get(name)
            .ok_or_else(|| CommandError::UnknownCommand(name.to_string()))?;
        build(args)
    }
}

fn build<O: Operation>(args: serde_json::Value) -> Result<Box<dyn Command>, CommandError> {
    let operation: O =
        serde_json::from_value(args).map_err(|error| CommandError::BadArguments {
            name: O::NAME.to_string(),
            message: error.to_string(),
        })?;
    Ok(Box::new(operation))
}

#[cfg(test)]
mod tests {
    use super::*;
    use livecraft_model::ids::IdAllocator;
    use livecraft_model::ids::TrackId;
    use livecraft_model::model::Project;
    use serde::{Deserialize, Serialize};
    use serde_json::json;

    /// A test-only edit with two arguments and nothing to undo.
    #[derive(Debug, Serialize, Deserialize)]
    struct Probe {
        track: TrackId,
        gain: f32,
    }

    impl Operation for Probe {
        const NAME: &'static str = "test.probe";
        const LABEL: &'static str = "Probe";

        fn perform(
            &mut self,
            _project: &mut Project,
            _ids: &mut IdAllocator,
        ) -> Result<(), CommandError> {
            Ok(())
        }

        fn revert(&mut self, _project: &mut Project) {}
    }

    /// A second test-only edit, so the listing has more than one name to order.
    #[derive(Debug, Serialize, Deserialize)]
    struct Alpha {}

    impl Operation for Alpha {
        const NAME: &'static str = "test.alpha";
        const LABEL: &'static str = "Alpha";

        fn perform(
            &mut self,
            _project: &mut Project,
            _ids: &mut IdAllocator,
        ) -> Result<(), CommandError> {
            Ok(())
        }

        fn revert(&mut self, _project: &mut Project) {}
    }

    fn registry() -> CommandRegistry {
        let mut registry = CommandRegistry::new();
        registry.register::<Probe>();
        registry.register::<Alpha>();
        registry
    }

    #[test]
    fn a_command_built_from_json_reports_the_same_arguments() {
        let args = json!({ "track": 3, "gain": -6.0 });
        let command = registry()
            .build("test.probe", args.clone())
            .expect("build probe");
        assert_eq!(command.name(), "test.probe");
        assert_eq!(command.label(), "Probe");
        assert_eq!(command.arguments(), args);
    }

    #[test]
    fn every_registered_name_round_trips_its_arguments() {
        let registry = registry();
        let samples = [
            ("test.alpha", json!({})),
            ("test.probe", json!({ "track": 1, "gain": 0.5 })),
        ];
        let names: Vec<&str> = registry.names().collect();
        let sampled: Vec<&str> = samples.iter().map(|(name, _)| *name).collect();
        assert_eq!(names, sampled, "every registered command needs a sample");
        for (name, args) in samples {
            let command = registry.build(name, args.clone()).expect("build sample");
            assert_eq!(command.arguments(), args, "round trip of `{name}`");
        }
    }

    #[test]
    fn an_unknown_name_is_an_error() {
        let error = registry().build("test.missing", json!({})).err();
        assert_eq!(
            error,
            Some(CommandError::UnknownCommand("test.missing".to_string()))
        );
    }

    #[test]
    fn arguments_that_do_not_decode_name_the_command() {
        let error = registry()
            .build("test.probe", json!({ "track": "one", "gain": 0.0 }))
            .err();
        assert!(matches!(
            error,
            Some(CommandError::BadArguments { ref name, .. }) if name == "test.probe"
        ));
    }

    #[test]
    fn names_come_back_sorted() {
        let names: Vec<&str> = registry().names().collect();
        assert_eq!(names, ["test.alpha", "test.probe"]);
    }
}
