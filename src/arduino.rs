use std::{collections::HashMap, fs};
use zed_extension_api::{self as zed, serde_json, settings::LspSettings, LanguageServerId, Result};

struct ArduinoExtension {
    cached_binary_path: Option<String>,
}

impl ArduinoExtension {
    fn language_server_binary_path(
        &mut self,
        language_server_id: &LanguageServerId,
        worktree: &zed::Worktree,
    ) -> Result<String> {
        // Check for explicit path override in settings
        if let Ok(lsp_settings) = LspSettings::for_worktree("arduino", worktree) {
            if let Some(binary) = lsp_settings.binary {
                if let Some(path) = binary.path {
                    // Note: If a custom path is provided, we assume it's correct
                    // and don't perform our download/versioning logic.
                    return Ok(path.clone());
                }
            }
        }

        // Check if the binary is already available in the system's PATH
        if let Some(path) = worktree.which("arduino_language_server") {
            return Ok(path);
        }

        // Check if we've cached a binary path from a previous download
        // and that it still exists
        if let Some(path) = &self.cached_binary_path {
            if fs::metadata(path).map_or(false, |stat| stat.is_file()) {
                return Ok(path.clone());
            }
        }

        // If none of the above, proceed with downloading the latest version
        zed::set_language_server_installation_status(
            language_server_id,
            &zed::LanguageServerInstallationStatus::CheckingForUpdate,
        );

        let release = zed::latest_github_release(
            "arduino/arduino-language-server",
            zed::GithubReleaseOptions {
                require_assets: true,
                pre_release: false,
            },
        )?;

        let (platform, arch) = zed::current_platform();

        // Determine the expected asset name based on platform and architecture
        // Note: This format matches the GitHub release asset names
        let asset_name = format!(
            "arduino-language-server_{}_{}_{}.tar.gz",
            release.version,
            match platform {
                zed::Os::Mac => "macOS",
                zed::Os::Linux => "Linux",
                zed::Os::Windows => "Windows",
            },
            match arch {
                zed::Architecture::Aarch64 => "ARM64",
                zed::Architecture::X86 => "32bit",
                zed::Architecture::X8664 => "64bit",
            },
        );

        let asset = release
            .assets
            .iter()
            .find(|asset| asset.name == asset_name)
            .ok_or_else(|| format!("no asset found matching {:?}", asset_name))?;

        // Define the version-specific directory name
        let version_dir = format!("arduino-language-server-{}", release.version);

        // Determine the expected name of the executable file within the extracted archive
        let binary_name = match platform {
            zed::Os::Mac | zed::Os::Linux => "arduino-language-server",
            zed::Os::Windows => "arduino-language-server.exe",
        };

        // Construct the full path to the binary *inside* the versioned directory
        let final_binary_path = format!("{}/{}", version_dir, binary_name);

        // Check if the binary already exists at the expected versioned path
        if !fs::metadata(&final_binary_path).map_or(false, |stat| stat.is_file()) {
            zed::set_language_server_installation_status(
                language_server_id,
                &zed::LanguageServerInstallationStatus::Downloading,
            );

            // Download the archive. The target path for download_file is the directory
            // where the archive should be extracted.
            zed::download_file(
                &asset.download_url,
                &version_dir,
                zed::DownloadedFileType::GzipTar,
            )
            .map_err(|e| format!("failed to download file: {e}"))?;

            // Clean up old versions: Remove any directories in the current download location
            // that are not the newly downloaded version directory.
            let entries =
                fs::read_dir(".").map_err(|e| format!("failed to list working directory {e}"))?;
            for entry in entries {
                let entry = entry.map_err(|e| format!("failed to load directory entry {e}"))?;
                let file_type = entry.file_type().map_err(|e| {
                    format!("failed to get file type for {:?}: {}", entry.path(), e)
                })?;

                if file_type.is_dir() {
                    if entry.file_name().to_str() != Some(&version_dir) {
                        // Ignore errors during cleanup as they aren't critical
                        fs::remove_dir_all(entry.path()).ok();
                    }
                }
            }

            // Make the downloaded binary executable
            zed::make_file_executable(&final_binary_path)?;
        }

        self.cached_binary_path = Some(final_binary_path.clone());
        Ok(final_binary_path)
    }
}

impl zed::Extension for ArduinoExtension {
    fn new() -> Self {
        Self {
            cached_binary_path: None,
        }
    }

    fn language_server_command(
        &mut self,
        language_server_id: &LanguageServerId,
        worktree: &zed::Worktree,
    ) -> Result<zed::Command> {
        // Get args and env from LSP settings first
        let mut args: Vec<String> = Vec::new();
        let mut env: HashMap<String, String> = HashMap::new();

        if let Ok(lsp_settings) = LspSettings::for_worktree("arduino", worktree) {
            if let Some(binary) = lsp_settings.binary {
                if let Some(binary_args) = binary.arguments {
                    args = binary_args;
                }

                if let Some(binary_env) = binary.env {
                    env = binary_env;
                }
            }
        }

        // Get the path to the language server binary
        let command_path = self.language_server_binary_path(language_server_id, worktree)?;

        // Check if the user already specified the -clangd flag in settings
        let user_specified_clangd = args.iter().any(|arg| arg == "-clangd");
        let user_specified_cli = args.iter().any(|arg| arg == "-cli");
        let user_specified_cli_config = args.iter().any(|arg| arg == "-cli-config");

        if !user_specified_cli_config {
            // Set the default cli-config path based on OS
            let cli_config_path = match zed::current_platform().0 {
                zed::Os::Mac => {
                    let home = std::env::home_dir().expect("Failed to get home directory");
                    home.join("Library/Arduino15/arduino-cli.yaml")
                }
                zed::Os::Linux => {
                    let home = std::env::home_dir().expect("Failed to get home directory");
                    home.join(".arduino15/arduino-cli.yaml")
                }
                zed::Os::Windows => {
                    let local_app_data =
                        std::env::var("LOCALAPPDATA").expect("LOCALAPPDATA not found");

                    let mut path = std::path::PathBuf::from(&local_app_data);
                    path.push("Arduino15");
                    path.push("arduino-cli.yaml");
                    path
                }
            };
            if cli_config_path.exists() {
                args.push("-cli-config".to_string());
                args.push(cli_config_path.to_string_lossy().to_string());
            }
        }

        if !user_specified_clangd {
            // User did not specify -clangd, try to find it automatically
            if let Some(clangd_path) = worktree.which("clangd") {
                // Add the flag and its value to the arguments
                args.push("-clangd".to_string());
                args.push(clangd_path);
            }
        }

        if !user_specified_cli {
            if let Some(cli_path) = worktree.which("arduino-cli") {
                args.push("-cli".to_string());
                args.push(cli_path);
            }
        }

        // Determine environment variables.
        // If environment variables were provided in settings, use those.
        // Otherwise, use shell_env on Mac/Linux as a default.
        if env.is_empty() {
            // Only apply default if no env was set in settings
            let default_env = match zed::current_platform().0 {
                zed::Os::Mac | zed::Os::Linux => worktree.shell_env(),
                zed::Os::Windows => Vec::new(), // Windows doesn't typically need shell_env
            };

            // Convert default_env (Vec<(String, String)>) to HashMap
            for (key, value) in default_env {
                env.insert(key, value);
            }
        }

        Ok(zed::Command {
            command: command_path,
            args,
            env: env.into_iter().collect(),
        })
    }

    fn language_server_workspace_configuration(
        &mut self,
        _language_server_id: &zed::LanguageServerId,
        worktree: &zed::Worktree,
    ) -> Result<Option<serde_json::Value>> {
        // This function provides the `workspace/configuration` response to the language server
        // Get the 'settings' section from the arduino LSP settings in Zed
        let settings = LspSettings::for_worktree("arduino", worktree)
            .ok()
            .and_then(|lsp_settings| lsp_settings.settings.clone())
            .unwrap_or_default();

        Ok(Some(settings))
    }
}

zed::register_extension!(ArduinoExtension);
