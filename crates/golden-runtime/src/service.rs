use std::{
    sync::{Arc, mpsc},
    thread::{self, JoinHandle},
};

use crate::{GenerationCompiler, GenerationSpec, RuntimeCompileError, RuntimeGeneration};

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct CompilationRequestId(pub u64);

pub struct CompilationResult {
    pub request: CompilationRequestId,
    pub generation: Result<Arc<RuntimeGeneration>, RuntimeCompileError>,
}

enum WorkerCommand {
    Compile(CompilationRequestId, GenerationSpec),
    Shutdown,
}

pub struct CompilationService {
    commands: mpsc::Sender<WorkerCommand>,
    results: mpsc::Receiver<CompilationResult>,
    worker: Option<JoinHandle<()>>,
    next_request: u64,
}

impl CompilationService {
    pub fn spawn() -> Self {
        let (command_tx, command_rx) = mpsc::channel();
        let (result_tx, result_rx) = mpsc::channel();
        let worker = thread::Builder::new()
            .name("golden-generation-compiler".into())
            .spawn(move || {
                let compiler = GenerationCompiler;
                while let Ok(command) = command_rx.recv() {
                    match command {
                        WorkerCommand::Compile(request, spec) => {
                            let generation = compiler.compile(spec).map(Arc::new);
                            if result_tx.send(CompilationResult { request, generation }).is_err() {
                                break;
                            }
                        }
                        WorkerCommand::Shutdown => break,
                    }
                }
            })
            .expect("generation compiler thread must start");
        Self {
            commands: command_tx,
            results: result_rx,
            worker: Some(worker),
            next_request: 1,
        }
    }

    pub fn request(&mut self, spec: GenerationSpec) -> CompilationRequestId {
        let request = CompilationRequestId(self.next_request);
        self.next_request = self
            .next_request
            .checked_add(1)
            .expect("compilation request space exhausted");
        self.commands
            .send(WorkerCommand::Compile(request, spec))
            .expect("generation compiler unexpectedly stopped");
        request
    }

    pub fn try_receive(&self) -> Option<CompilationResult> {
        self.results.try_recv().ok()
    }
}

impl Drop for CompilationService {
    fn drop(&mut self) {
        let _ = self.commands.send(WorkerCommand::Shutdown);
        if let Some(worker) = self.worker.take() {
            let _ = worker.join();
        }
    }
}
