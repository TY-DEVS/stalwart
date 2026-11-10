use std::{
    borrow::Cow,
    collections::hash_map::Entry,
    net::{IpAddr, SocketAddr},
    str::FromStr,
    sync::Arc,
    time::Duration,
};

use ahash::AHashMap;
use compact_str::{CompactString, ToCompactString, format_compact};
use directory::{
    bind::BindCredential,
    directory::{
        Authority, BaseDn, Directory, DirectoryInternal, DirectoryLdap, DirectoryOidc,
        DirectorySql, DirectoryStatic, DirectoryTrust,
    },
    identity::Identity,
};
use email::account::Account;
use jmap_proto::core::error::SetError;
use registry::{
    Map, ObjectType, RegistryObject, RegistryRead, RegistryWrite, Value,
    bootstrap::{Bootstrap, SetType},
    set::{SetAction, SetContext},
};
use store::{Store, StoreQueue};
use tracing::{Instrument, info_span};
use types::{
    application::{Application, AppOriginPolicy},
    domain::Domain,
    group::Group,
    network::{
        HttpListener, HttpRouter, NetworkListener, NetworkListenerProtocol,
        NetworkListenerTlsOption, ReverseProxy,
    },
    quota::QuotaRule,
    rule::Rule,
    settings::{
        AdminGroup, AdminUser, AuthenticationSettings, DirectorySettings, JmapSettings,
        LogLevel, LoggingSettings, ServerConfiguration, SmtpDsnSettings, SmtpInboundSettings,
        SmtpOutboundSettings, SystemSettings, WebAdminSettings,
    },
};

pub async fn insert_defaults(bp: &mut Bootstrap) -> trc::Result<()> {
    insert_safe_defaults(bp).await?;
    insert_demo_defaults(bp).await?;

    Ok(())
}

async fn insert_safe_defaults(bp: &mut Bootstrap) -> trc::Result<()> {
    let is_recovery_mode = bp.registry.is_recovery_mode();
    let is_bootstrap_mode = bp.registry.is_bootstrap_mode();

    #[cfg(not(stalwart_tests))]
    if bp.registry.count_object(ObjectType::Application).await? == 0 {
        bp.registry
            .write(RegistryWrite::insert(
                &Application {
                    description: "Stalwart Web Interface".to_string(),
                    enable: false,
                    #[cfg(not(feature = "dev_mode"))]
                    resource_url:
                        "https://github.com/stalwartlabs/webui/releases/download/v2/webui.zip"
                            .into(),
                    #[cfg(feature = "dev_mode")]
                    resource_url: "file://../webui/.ignore/webui.zip".into(),
                    url_prefix: Map::new(vec!["/admin".into(), "/account".into()]),
                    ..Default::default()
                }
                .into(),
            ))
            .await?;
    }

    if is_bootstrap_mode {
        #[cfg(not(any(feature = "dev_mode", stalwart_tests)))]
        if bp.registry.count_object(ObjectType::SystemSettings).await? == 0 {
            bp.registry
                .write(RegistryWrite::insert(
                    &SystemSettings {
                        default_hostname: bp.registry.local_hostname().to_string(),
                        ..Default::default()
                    }
                    .into(),
                ))
                .await?;
        }

        if bp.registry.count_object(ObjectType::Store).await? == 0 {
            bp.registry
                .write(RegistryWrite::insert(
                    &Store {
                        description: "Default storage backend".to_string().into(),
                        name: "default".into(),
                        data: "default".into(),
                        fts: "default".into(),
                        lookup: "default".into(),
                        blob: "default".into(),
                    }
                    .into(),
                ))
                .await?;
        }

        if bp.registry.count_object(ObjectType::Directory).await? == 0 {
            bp.registry
                .write(RegistryWrite::insert(
                    &Directory {
                        description: "Internal directory".to_string().into(),
                        name: "internal".into(),
                        details: DirectoryInternal {
                            store: "default".into(),
                        }
                        .into(),
                        ..Default::default()
                    }
                    .into(),
                ))
                .await?;
        }

        if bp.registry.count_object(ObjectType::StoreQueue).await? == 0 {
            bp.registry
                .write(RegistryWrite::insert(
                    &StoreQueue {
                        description: "Default outbound delivery queue".to_string().into(),
                        name: "outbound".into(),
                        store: "default".into(),
                        schedule: "outbound".into(),
                        route: None,
                        report: None,
                        subqueues: Map::default(),
                    }
                    .into(),
                ))
                .await?;
            bp.registry
                .write(RegistryWrite::insert(
                    &StoreQueue {
                        description: "DMARC and TLS report delivery queue".to_string().into(),
                        name: "report".into(),
                        store: "default".into(),
                        schedule: "report".into(),
                        route: None,
                        report: None,
                        subqueues: Map::default(),
                    }
                    .into(),
                ))
                .await?;
        }

        if bp
            .registry
            .count_object(ObjectType::SmtpOutboundSettings)
            .await?
            == 0
        {
            bp.registry
                .write(RegistryWrite::insert(
                    &SmtpOutboundSettings {
                        default_queue: "outbound".into(),
                        ..Default::default()
                    }
                    .into(),
                ))
                .await?;
        }

        if bp.registry.count_object(ObjectType::Schedule).await? == 0 {
            bp.registry
                .write(RegistryWrite::insert(
                    &types::schedule::Schedule {
                        name: "outbound".into(),
                        description: "Default outbound delivery schedule".to_string().into(),
                        strategy: types::schedule::Strategy::Linear,
                        retry: vec![
                            Duration::from_secs(60),
                            Duration::from_secs(120),
                            Duration::from_secs(300),
                            Duration::from_secs(600),
                            Duration::from_secs(1800),
                            Duration::from_secs(3600),
                            Duration::from_secs(7200),
                            Duration::from_secs(14400),
                        ],
                    }
                    .into(),
                ))
                .await?;
            bp.registry
                .write(RegistryWrite::insert(
                    &types::schedule::Schedule {
                        name: "report".into(),
                        description: "DMARC and TLS report delivery schedule".to_string().into(),
                        strategy: types::schedule::Strategy::Linear,
                        retry: vec![
                            Duration::from_secs(3600),
                            Duration::from_secs(7200),
                            Duration::from_secs(14400),
                            Duration::from_secs(28800),
                            Duration::from_secs(57600),
                        ],
                    }
                    .into(),
                ))
                .await?;
        }

        if bp.registry.count_object(ObjectType::DirectorySettings).await? == 0 {
            bp.registry
                .write(RegistryWrite::insert(
                    &DirectorySettings {
                        name: "internal".into(),
                        ..Default::default()
                    }
                    .into(),
                ))
                .await?;
        }

        if bp.registry.count_object(ObjectType::Group).await? == 0 {
            bp.registry
                .write(RegistryWrite::insert(
                    &Group {
                        name: "admins".into(),
                        description: "Administrators".to_string().into(),
                        roles: vec!["admin".into()],
                        members: vec![],
                    }
                    .into(),
                ))
                .await?;
        }

        if bp.registry.count_object(ObjectType::AdminGroup).await? == 0 {
            bp.registry
                .write(RegistryWrite::insert(
                    &AdminGroup {
                        name: "admins".into(),
                        description: "Administrators".to_string().into(),
                        superadmin: true,
                        permissions: vec![],
                        roles: vec![],
                    }
                    .into(),
                ))
                .await?;
        }

        if bp.registry.count_object(ObjectType::QuotaRule).await? == 0 {
            bp.registry
                .write(RegistryWrite::insert(
                    &QuotaRule {
                        name: "default".into(),
                        description: "Default quota rule".to_string().into(),
                        messages: None,
                        storage: 10 * 1024 * 1024 * 1024,
                    }
                    .into(),
                ))
                .await?;
        }

        if bp.registry.count_object(ObjectType::Rule).await? == 0 {
            bp.registry
                .write(RegistryWrite::insert(
                    &Rule {
                        name: "quota".into(),
                        description: "Assign default quota".to_string().into(),
                        rule: "quota('default')".into(),
                    }
                    .into(),
                ))
                .await?;
        }

        #[cfg(not(stalwart_tests))]
        if bp
            .registry
            .count_object(ObjectType::NetworkListener)
            .await?
            == 0
        {
            for (protocol, name, port, implicit_tls) in [
                (NetworkListenerProtocol::Smtp, "smtp", 25, false),
                (NetworkListenerProtocol::Smtp, "submissions", 465, true),
                (NetworkListenerProtocol::Imap, "imaps", 993, true),
                (NetworkListenerProtocol::Pop3, "pop3s", 995, true),
                (NetworkListenerProtocol::ManageSieve, "sieve", 4190, false),
                (NetworkListenerProtocol::Http, "https", 443, true),
                (NetworkListenerProtocol::Http, "http", 8080, false),
            ] {
                bp.registry
                    .write(RegistryWrite::insert(
                        &NetworkListener {
                            address: SocketAddr::from_str(&format!("[::]:{port}")).unwrap(),
                            protocol,
                            name: name.into(),
                            tls: if implicit_tls {
                                NetworkListenerTlsOption::Implicit
                            } else {
                                NetworkListenerTlsOption::None
                            },
                            ..Default::default()
                        }
                        .into(),
                    ))
                    .await?;
            }
        }
    }

    Ok(())
}

async fn insert_demo_defaults(_bp: &mut Bootstrap) -> trc::Result<()> {
    Ok(())
}
