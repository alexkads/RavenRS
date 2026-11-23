# The package of Raven CI. Made by pacote/gerar-rpm.sh, which builds the release binaries and
# the panel for the browser first and passes the version; it is not meant to run by hand.

Name:           raven-ci
Version:        %{versao}
Release:        %{lancamento}%{?dist}
Summary:        A CI/CD server of its own, made of plugins, in a fraction of the memory of Jenkins
License:        MIT OR Apache-2.0
URL:            https://github.com/alexkads/RavenRS/tree/master/tools/raven-ci
Source0:        raven-ci
Source1:        raven-ci-agent
Source2:        raven-ci.service
Source3:        raven-ci.sysusers
Source4:        painel.tar
Source5:        README.md
Source6:        agentes-de-outro-usuario.conf
ExclusiveArch:  x86_64

BuildRequires:  systemd-rpm-macros
Requires:       systemd
Requires:       git
%{?systemd_requires}
%{?sysusers_requires_compat}

%description
Raven CI is a CI/CD server whose core holds only the queue, the model of jobs and builds and the
register of plugins: the pipeline formats (.raven-ci.toml and the declarative Jenkinsfile), the
steps (sh, cargo, docker, kubernetes), git, GitHub and Azure DevOps, the triggers, the
credentials, the publishers, the notices and the agents are all plugins. It comes with the panel
for the browser at /painel/ and the agent raven-ci-agent.

The service raven-ci runs as the user raven-ci, with its data in /var/lib/raven-ci, listening on
127.0.0.1:8090, with MemoryMax=256M. The first person made with
`raven-ci --dados /var/lib/raven-ci usuario criar NOME` is an administrator.

%prep
# Nothing to prepare: the binaries and the panel come built.

%build
# Idem.

%install
install -Dpm 0755 %{SOURCE0} %{buildroot}%{_bindir}/raven-ci
install -Dpm 0755 %{SOURCE1} %{buildroot}%{_bindir}/raven-ci-agent
install -Dpm 0644 %{SOURCE2} %{buildroot}%{_unitdir}/raven-ci.service
install -Dpm 0644 %{SOURCE3} %{buildroot}%{_sysusersdir}/raven-ci.conf
install -dm 0755 %{buildroot}%{_datadir}/raven-ci/painel
tar -xf %{SOURCE4} -C %{buildroot}%{_datadir}/raven-ci/painel
install -dm 0750 %{buildroot}%{_sharedstatedir}/raven-ci
install -Dpm 0644 %{SOURCE5} %{buildroot}%{_docdir}/%{name}/README.md
install -Dpm 0644 %{SOURCE6} %{buildroot}%{_datadir}/raven-ci/agentes-de-outro-usuario.conf

%pre
%sysusers_create_compat %{SOURCE3}

%post
%systemd_post raven-ci.service
if [ $1 -eq 1 ]; then
    systemctl enable --now raven-ci.service >/dev/null 2>&1 || :
fi

%preun
%systemd_preun raven-ci.service

%postun
# On an upgrade, it starts again with the new binary; the data stays.
%systemd_postun_with_restart raven-ci.service

%files
%{_bindir}/raven-ci
%{_bindir}/raven-ci-agent
%{_unitdir}/raven-ci.service
%{_sysusersdir}/raven-ci.conf
%{_datadir}/raven-ci/painel
%{_datadir}/raven-ci/agentes-de-outro-usuario.conf
%attr(0750,raven-ci,raven-ci) %dir %{_sharedstatedir}/raven-ci
%doc %{_docdir}/%{name}/README.md

%changelog
* Mon Nov 23 2026 Alex <alexkads@users.noreply.github.com> - 0.1.5-1
- CPUWeight and IOWeight in place of Nice=10; the drop-in agentes-de-outro-usuario.conf

* Sun Nov 22 2026 Alex <alexkads@users.noreply.github.com> - 0.1.3-1
- First package: the server as a service, the agent and the panel for the browser
