# Raven CI installed from its RPM in a clean Fedora: a controlled place to try a package before
# a machine gets it. The RPM is the one pacote/gerar-rpm.sh made, put beside this file:
#
#   cp ../rpm/raven-ci-*.rpm . && docker compose up -d --build
#
# A container has no systemd, so the service of the package is not started here: the container
# runs the same command the unit runs, as the same user, with the same data folder.
FROM registry.fedoraproject.org/fedora:45

COPY raven-ci-*.rpm /tmp/
RUN dnf -y install /tmp/raven-ci-*.rpm && dnf clean all && rm -f /tmp/raven-ci-*.rpm \
 && raven-ci --versao

USER raven-ci
WORKDIR /var/lib/raven-ci
VOLUME /var/lib/raven-ci
EXPOSE 8090
HEALTHCHECK --interval=30s --timeout=5s --start-period=10s \
  CMD curl -fsS http://127.0.0.1:8090/health || exit 1
ENTRYPOINT ["raven-ci", "--dados", "/var/lib/raven-ci"]
CMD ["servir", "--endereco", "0.0.0.0:8090", "--executores", "1", "--painel", "/usr/share/raven-ci/painel"]
