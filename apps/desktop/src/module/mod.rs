//! UI modules: one per domain that has moved out of the shell.

pub mod architecture;
pub mod automation;
pub mod composer;
pub mod documents;
pub mod extensions;
pub mod extensions_browser;
pub mod memory;
pub mod roadmap;
pub mod settings;
pub mod task;
pub mod timeline;

use lilia_kernel::{Feature, FeatureContext, FeatureId, KernelError};

use crate::application::ApplicationWorkspaceSurface;
use crate::runtime_shell::ShellProjectPage;
use crate::ui_module::{UiModuleContext, UiModules};

pub(crate) fn conversation_is_visible(cx: &UiModuleContext<'_>) -> bool {
    if cx.shows_surface(ApplicationWorkspaceSurface::Settings)
        || cx.shows_surface(ApplicationWorkspaceSurface::Automations)
        || cx.shows_surface(ApplicationWorkspaceSurface::Projects)
    {
        return false;
    }
    !(cx.shows(ShellProjectPage::Sessions)
        || cx.shows(ShellProjectPage::Overview)
        || cx.shows(ShellProjectPage::Clone)
        || cx.shows(ShellProjectPage::Roadmap)
        || cx.shows(ShellProjectPage::Memory)
        || cx.shows(ShellProjectPage::Architecture)
        || cx.shows(ShellProjectPage::Settings)
        || cx.shows(ShellProjectPage::Files))
}

pub struct ShellUiFeature;

impl Feature for ShellUiFeature {
    fn id(&self) -> FeatureId {
        FeatureId::new("lilia.shell.ui").expect("the shell ui feature id is not blank")
    }

    fn mount(&self, cx: &mut FeatureContext<'_>) -> Result<(), KernelError> {
        cx.contribute::<UiModules>(Box::new(|_| {
            Ok(Box::new(architecture::ArchitectureModule::default()))
        }));
        cx.contribute::<UiModules>(Box::new(|_| {
            Ok(Box::new(roadmap::RoadmapModule::default()))
        }));
        cx.contribute::<UiModules>(Box::new(|_| Ok(Box::new(memory::MemoryModule::default()))));
        cx.contribute::<UiModules>(Box::new(|_| {
            Ok(Box::new(composer::ComposerModule::default()))
        }));
        cx.contribute::<UiModules>(Box::new(|_| {
            Ok(Box::new(extensions::ExtensionsModule::default()))
        }));
        cx.contribute::<UiModules>(Box::new(|_| Ok(Box::new(task::TaskModule::default()))));
        cx.contribute::<UiModules>(Box::new(|_| {
            Ok(Box::new(documents::DocumentsModule::default()))
        }));
        cx.contribute::<UiModules>(Box::new(|_| {
            Ok(Box::new(timeline::TimelineModule::default()))
        }));
        cx.contribute::<UiModules>(Box::new(|_| {
            Ok(Box::new(settings::SettingsModule::default()))
        }));
        cx.contribute::<UiModules>(Box::new(|cx| {
            let service = cx
                .kernel()
                .service::<lilia_feature_automation::AutomationServiceKey>()
                .map_err(|error| error.to_string())?;
            Ok(Box::new(automation::controller::AutomationController::new(
                cx.window(),
                service,
                cx.kernel().jobs().clone(),
            )))
        }));
        Ok(())
    }
}

#[cfg(test)]
mod project_refresh_tests;
