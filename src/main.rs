use lygos::code_gen::CodeGenerator;
use lygos_ast::type_check::TypeChecker;
use lygos_llvm::{builder::IRBuilder, context::Context, module::Module};
use lygos_logging::{ErrorLabel, Logger};
use lygos_util::cli::{CliArgs, EmitType};
use std::{fs, process::Command};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    color_eyre::install()?;
    let args = CliArgs::parse();
    let input = args.input.as_deref().ok_or("No input file")?;
    let root = lygos::modules::load(input).unwrap_or_else(|error| {
        let mut labels = vec![ErrorLabel::from(&error.loc, &error.message)];
        labels.extend(
            error
                .related
                .iter()
                .map(|(loc, message)| ErrorLabel::from(loc, message)),
        );
        Logger::diagnostic(&error.message, &labels, error.help.as_deref(), None)
    });
    if args.emit_type == Some(EmitType::AST) {
        let ast = format!("{root:#?}\n");
        if let Some(output) = args.output {
            fs::write(output, ast)?;
        } else {
            print!("{ast}");
        }
        return Ok(());
    }
    let analysis = TypeChecker::analyze(&root);
    let errors = analysis.as_ref().err().map(Vec::as_slice).unwrap_or(&[]);
    for error in errors {
        let mut labels = vec![ErrorLabel::from(
            &error.loc,
            error.primary_label.as_deref().unwrap_or(&error.message),
        )];
        labels.extend(
            error
                .related
                .iter()
                .map(|(loc, message)| ErrorLabel::from(loc, message)),
        );
        Logger::emit_diagnostic(&error.message, &labels, error.help.as_deref(), None);
    }
    if !errors.is_empty() {
        eprintln!(
            "compilation stopped after {} error{}",
            errors.len(),
            if errors.len() == 1 { "" } else { "s" }
        );
        std::process::exit(1);
    }
    if args.check {
        return Ok(());
    }
    let typed = analysis.unwrap();
    if args.emit_type == Some(EmitType::TypedAst) {
        let dump = format!("{typed:#?}\n");
        if let Some(output) = args.output {
            fs::write(output, dump)?;
        } else {
            print!("{dump}");
        }
        return Ok(());
    }

    Context::init_all();
    let triple = Context::get_default_target_triple();
    let target = Context::lookup_target(&triple);
    let machine = Context::create_target_machine(target, triple.as_str(), "generic", "");
    let context = Context::new();
    let mut module = Module::new(input.display().to_string(), &context);
    module.set_data_layout(&machine);
    module.set_target_triple(&triple);
    let builder = IRBuilder::new(&context);
    CodeGenerator::new(&context, &module, &builder).generate(&typed)?;
    if args.emit_type == Some(EmitType::LlvmIr) {
        let ir = module.print();
        if let Some(output) = args.output {
            fs::write(output, ir)?;
        } else {
            print!("{ir}");
        }
        return Ok(());
    }
    let object = input.with_extension("o");
    if !Context::emit_obj_file(object.display().to_string(), &module, &machine) {
        return Err("failed to emit object file".into());
    }
    let output = args
        .output
        .unwrap_or_else(|| input.with_extension(if cfg!(windows) { "exe" } else { "out" }));
    let mut linker = Command::new("clang");
    #[cfg(windows)]
    if let Some(tool) = cc::windows_registry::find_tool(triple.as_str(), "link.exe") {
        // Cargo's toolchain discovery includes both the CRT and Windows SDK.
        // Apply it only to this child process, keeping ordinary shells usable.
        linker.envs(tool.env().iter().cloned());
    }
    let status = linker.arg("-o").arg(output).arg(&object).status()?;
    if !status.success() {
        return Err(format!("linker failed: {status}").into());
    }
    Ok(())
}
