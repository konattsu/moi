mod defaults {
    pub(super) const FOLDER_NAME: &str = "environments";
    pub(super) const SOURCE: &str = "https://github.com/konattsu/moi.git";
    pub(super) const INSTALL_SOURCE: &str =
        "https://raw.githubusercontent.com/konattsu/moi/main/";
    pub(super) const INSTALL_SCRIPT: &str = "install.sh";
    pub(super) const CONFIG_DIR: &str = ".config/moi";
    pub(super) const CONFIG_FILE: &str = "config.toml";
}

#[derive(Debug)]
pub(crate) struct Settings {
    environment: String,
    folder_name: String,
    source: String,
}

#[derive(Debug, Clone, Copy)]
pub(crate) struct SettingsOverrides<'a> {
    pub(crate) environment: Option<&'a str>,
    pub(crate) folder_name: Option<&'a str>,
    pub(crate) source: Option<&'a str>,
}

#[derive(Debug)]
pub(crate) struct InstallCommandSettings {
    environment: String,
    folder_name: String,
    source: String,
    install_source: String,
    install_script: String,
}

#[derive(Debug, Clone, Copy)]
pub(crate) struct InstallCommandOverrides<'a> {
    pub(crate) environment: Option<&'a str>,
    pub(crate) folder_name: Option<&'a str>,
    pub(crate) source: Option<&'a str>,
    pub(crate) install_source: Option<&'a str>,
    pub(crate) install_script: Option<&'a str>,
}

#[derive(Debug, Default, serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct ConfigFile {
    default_environment: Option<String>,
    default_folder_name: Option<String>,
    default_source: Option<String>,
    default_install_source: Option<String>,
    default_install_script: Option<String>,
}

#[derive(Debug)]
struct EnvOverrides {
    environment: Option<String>,
    folder_name: Option<String>,
    source: Option<String>,
    install_source: Option<String>,
    install_script: Option<String>,
}

#[derive(Debug, Clone, Copy)]
struct SettingSources<'a> {
    cli: Option<&'a str>,
    env: Option<&'a str>,
    file: Option<&'a str>,
}

impl Settings {
    pub(crate) fn resolve(
        cli: SettingsOverrides<'_>,
    ) -> std::result::Result<Self, crate::error::MoiError> {
        let config_path = config_path()?;
        let config = read_config_file(&config_path)?;
        let env = EnvOverrides::read();
        let environment = resolve_setting(
            SettingSources {
                cli: cli.environment,
                env: env.environment.as_deref(),
                file: config.default_environment.as_deref(),
            },
            "MOI_ENVIRONMENT",
            "environment",
            None,
            &config_path,
        )?;
        let folder_name = validate_folder_name(&resolve_setting(
            SettingSources {
                cli: cli.folder_name,
                env: env.folder_name.as_deref(),
                file: config.default_folder_name.as_deref(),
            },
            "MOI_FOLDER_NAME",
            "folder_name",
            Some(defaults::FOLDER_NAME),
            &config_path,
        )?)?;
        let source = validate_source(&resolve_setting(
            SettingSources {
                cli: cli.source,
                env: env.source.as_deref(),
                file: config.default_source.as_deref(),
            },
            "MOI_SOURCE",
            "source",
            Some(defaults::SOURCE),
            &config_path,
        )?)?;
        crate::path::name(&environment).map_err(crate::error::MoiError::config)?;
        Ok(Self {
            environment,
            folder_name,
            source,
        })
    }

    pub(crate) fn environment(&self) -> &str {
        &self.environment
    }
    pub(crate) fn folder_name(&self) -> &str {
        &self.folder_name
    }
    pub(crate) fn source(&self) -> &str {
        &self.source
    }
}

impl InstallCommandSettings {
    pub(crate) fn resolve(
        cli: InstallCommandOverrides<'_>,
    ) -> std::result::Result<Self, crate::error::MoiError> {
        let config_path = config_path()?;
        let config = read_config_file(&config_path)?;
        let env = EnvOverrides::read();
        let environment = cli
            .environment
            .filter(|value| !value.is_empty())
            .map(str::to_string)
            .ok_or_else(|| {
                crate::error::MoiError::config(
                    "missing required setting: environment (use --environment / -e)",
                )
            })?;
        let folder_name = validate_folder_name(&resolve_setting(
            SettingSources {
                cli: cli.folder_name,
                env: env.folder_name.as_deref(),
                file: config.default_folder_name.as_deref(),
            },
            "MOI_FOLDER_NAME",
            "folder_name",
            Some(defaults::FOLDER_NAME),
            &config_path,
        )?)?;
        let source = validate_source(&resolve_setting(
            SettingSources {
                cli: cli.source,
                env: env.source.as_deref(),
                file: config.default_source.as_deref(),
            },
            "MOI_SOURCE",
            "source",
            Some(defaults::SOURCE),
            &config_path,
        )?)?;
        let install_source = validate_install_source(&resolve_setting(
            SettingSources {
                cli: cli.install_source,
                env: env.install_source.as_deref(),
                file: config.default_install_source.as_deref(),
            },
            "MOI_INSTALL_SOURCE",
            "install_source",
            Some(defaults::INSTALL_SOURCE),
            &config_path,
        )?)?;
        let install_script = validate_install_script(&resolve_setting(
            SettingSources {
                cli: cli.install_script,
                env: env.install_script.as_deref(),
                file: config.default_install_script.as_deref(),
            },
            "MOI_INSTALL_SCRIPT",
            "install_script",
            Some(defaults::INSTALL_SCRIPT),
            &config_path,
        )?)?;
        crate::path::name(&environment).map_err(crate::error::MoiError::config)?;
        Ok(Self {
            environment,
            folder_name,
            source,
            install_source,
            install_script,
        })
    }

    pub(crate) fn environment(&self) -> &str {
        &self.environment
    }
    pub(crate) fn folder_name(&self) -> &str {
        &self.folder_name
    }
    pub(crate) fn source(&self) -> &str {
        &self.source
    }
    pub(crate) fn install_source(&self) -> &str {
        &self.install_source
    }
    pub(crate) fn install_script(&self) -> &str {
        &self.install_script
    }
}

impl EnvOverrides {
    fn read() -> Self {
        Self {
            environment: read_env("MOI_ENVIRONMENT"),
            folder_name: read_env("MOI_FOLDER_NAME"),
            source: read_env("MOI_SOURCE"),
            install_source: read_env("MOI_INSTALL_SOURCE"),
            install_script: read_env("MOI_INSTALL_SCRIPT"),
        }
    }
}

pub(crate) fn config_path()
-> std::result::Result<std::path::PathBuf, crate::error::MoiError> {
    Ok(crate::path::home_dir()?
        .join(defaults::CONFIG_DIR)
        .join(defaults::CONFIG_FILE))
}

fn read_config_file(
    path: &std::path::Path,
) -> std::result::Result<ConfigFile, crate::error::MoiError> {
    if crate::path::metadata(path)?.is_none() {
        return Ok(ConfigFile::default());
    }
    let content = std::fs::read_to_string(path)
        .map_err(|source| crate::error::MoiError::io(path, source))?;
    let config: ConfigFile =
        toml::from_str(&content).map_err(|source| crate::error::MoiError::Toml {
            path: path.to_path_buf(),
            source,
        })?;
    validate_optional_string(&config.default_environment, path, "default_environment")?;
    validate_optional_string(&config.default_folder_name, path, "default_folder_name")?;
    validate_optional_string(&config.default_source, path, "default_source")?;
    validate_optional_string(
        &config.default_install_source,
        path,
        "default_install_source",
    )?;
    validate_optional_string(
        &config.default_install_script,
        path,
        "default_install_script",
    )?;
    Ok(config)
}

fn validate_optional_string(
    value: &Option<String>,
    path: &std::path::Path,
    key: &str,
) -> std::result::Result<(), crate::error::MoiError> {
    if value.as_deref() == Some("") {
        return Err(crate::error::MoiError::config(format!(
            "{}: {key} must be a non-empty string",
            path.display()
        )));
    }
    Ok(())
}

fn read_env(name: &str) -> Option<String> {
    std::env::var(name).ok().filter(|value| !value.is_empty())
}

fn resolve_setting(
    sources: SettingSources<'_>,
    env_name: &str,
    display_name: &str,
    default: Option<&str>,
    config_path: &std::path::Path,
) -> std::result::Result<String, crate::error::MoiError> {
    let value = sources
        .cli
        .map(str::to_string)
        .or_else(|| sources.env.map(str::to_string))
        .or_else(|| {
            sources
                .file
                .filter(|val| !val.is_empty())
                .map(str::to_string)
        })
        .or_else(|| default.map(str::to_string));
    value.ok_or_else(|| {
        crate::error::MoiError::config(format!(
            "missing required setting: {display_name} (use --{} or {env_name} or {})",
            display_name.replace('_', "-"),
            config_path.display()
        ))
    })
}

fn validate_source(value: &str) -> Result<String, crate::error::MoiError> {
    if value.starts_with("https://") {
        crate::path::https(value).map_err(crate::error::MoiError::config)?;
    } else {
        let parsed = url::Url::parse(value)
            .map_err(|e| crate::error::MoiError::config(e.to_string()))?;
        if parsed.scheme() != "file"
            || !value.starts_with("file:///")
            || parsed.to_file_path().is_err()
            || parsed.query().is_some()
            || parsed.fragment().is_some()
        {
            return Err(crate::error::MoiError::config(
                "source must be HTTPS or file:/// absolute checkout",
            ));
        }
    }
    Ok(value.into())
}

fn validate_install_source(value: &str) -> Result<String, crate::error::MoiError> {
    crate::path::text(value).map_err(crate::error::MoiError::config)?;
    if value.starts_with("https://") {
        crate::path::https(value).map_err(crate::error::MoiError::config)?;
        return Ok(value.into());
    }
    if value.starts_with("file:") {
        return validate_source(value);
    }
    let path = if value.starts_with('~') {
        crate::path::expand(value)?
    } else {
        let path = std::path::PathBuf::from(value);
        if path.is_absolute() {
            crate::path::destination(value).map_err(crate::error::MoiError::config)?;
            path
        } else {
            crate::path::relative(value).map_err(crate::error::MoiError::config)?;
            std::env::current_dir()
                .map_err(|e| crate::error::MoiError::io("cwd", e))?
                .join(path)
        }
    };
    Ok(path.to_string_lossy().into_owned())
}

fn validate_install_script(value: &str) -> Result<String, crate::error::MoiError> {
    crate::path::relative(value).map_err(crate::error::MoiError::config)?;
    Ok(value.into())
}

fn validate_folder_name(value: &str) -> Result<String, crate::error::MoiError> {
    crate::path::relative(value).map_err(crate::error::MoiError::config)?;
    Ok(value.into())
}
