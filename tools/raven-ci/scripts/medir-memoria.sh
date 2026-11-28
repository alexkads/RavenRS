#!/usr/bin/env bash
# Measures the memory of Raven CI (V-02 of #803): the resident memory (RSS) of the server and of
# the agent, idle and with one build running on the agent, as `ps` says it, in KiB.
#
#   tools/raven-ci/scripts/medir-memoria.sh [PASTA_DOS_BINARIOS] [PASTA_TEMPORARIA]
#
# PASTA_DOS_BINARIOS holds `raven-ci` and `raven-ci-agent` built in release
# (`cargo build --release --manifest-path tools/raven-ci/Cargo.toml -p raven-ci -p raven-ci-agent`), by default `target/release`. Everything it
# makes goes to PASTA_TEMPORARIA (a folder of the system's temporary one by default) and is
# removed at the end, as the processes it starts. It is bash, which macOS and Linux both have
# (the zsh of 0.1.3 is not on a Fedora server, where the RPM runs).
set -u
B=${1:-target/release}
S=${2:-${TMPDIR:-/tmp}/raven-ci-medida-$$}
D=$S/dados
mkdir -p "$D"
SENHA=senha-da-medida-123

rss() { ps -o rss= -p "$1" 2>/dev/null | tr -d ' '; }
# A mediana de N amostras de 1 s.
mediana() {
  local pid=$1 n=$2 i vals=()
  for i in $(seq 1 "$n"); do vals+=("$(rss "$pid")"); sleep 1; done
  printf '%s\n' "${vals[@]}" | sort -n | sed -n "$(( (n + 1) / 2 ))p"
}

echo "$SENHA" | "$B/raven-ci" --dados "$D" usuario criar ana --papel Leitores >/dev/null
"$B/raven-ci" --dados "$D" servir --endereco 127.0.0.1:0 > "$S/servidor.log" 2>&1 &
SRV=$!
for i in $(seq 1 50); do grep -q "http://" "$S/servidor.log" && break; sleep 0.2; done
URL=$(grep -o 'http://[0-9.:]*' "$S/servidor.log" | head -1)
echo "servidor $SRV em $URL"
sleep 3
echo "1. servidor parado, sem agente: $(mediana $SRV 5) KiB (mediana de 5)"

TOKEN=$("$B/raven-ci" --dados "$D" agentes token medida-1 --rotulos medida 2>&1 | grep -o 'rva_[A-Za-z0-9_-]*' | head -1)
RAVEN_CI_AGENT_TOKEN=$TOKEN "$B/raven-ci-agent" --servidor "$URL" --nome medida-1 --rotulos medida --pasta "$D/agente" > "$S/agente.log" 2>&1 &
AGT=$!
sleep 4
echo "2. agente parado, conectado: $(mediana $AGT 5) KiB; servidor com o agente: $(mediana $SRV 5) KiB"

CHAVE=$(curl -s -X POST "$URL/api/auth/token" -H 'Content-Type: application/json' -d "{\"name\":\"ana\",\"password\":\"$SENHA\"}" | grep -o '"token":"[^"]*"' | cut -d'"' -f4)
PIPE='[pipeline]\nagent = { label = \"medida\" }\n\n[[stages]]\nname = \"Build\"\nsteps = [\"echo comecou\", \"sleep 8\", \"echo terminou\"]\n'
curl -s -X POST "$URL/api/ci/jobs" -H "Authorization: Bearer $CHAVE" -H 'Content-Type: application/json' \
  -d "{\"name\":\"medida\",\"pipeline\":\"$PIPE\",\"settings\":{\"triggers\":[{\"id\":\"api\"}]}}" >/dev/null
curl -s -X POST "$URL/api/ci/run" -H "Authorization: Bearer $CHAVE" -H 'Content-Type: application/json' -d '{"job":"medida"}' > "$S/run.json"
echo "build pedido: $(cat $S/run.json)"
pico_s=0; pico_a=0
for i in $(seq 1 60); do
  s=$(rss $SRV); a=$(rss $AGT)
  (( s > pico_s )) && pico_s=$s
  (( a > pico_a )) && pico_a=$a
  sleep 0.2
done
STATUS=$(curl -s "$URL/api/ci/builds?job=medida" -H "Authorization: Bearer $CHAVE" | grep -o '"status":"[a-z_]*"' | head -1)
echo "3. durante o build: pico do servidor $pico_s KiB, pico do agente $pico_a KiB (amostras de 200 ms por 12 s); $STATUS"
sleep 2
echo "4. depois do build: servidor $(mediana $SRV 3) KiB, agente $(mediana $AGT 3) KiB"
echo "binários: raven-ci $(( $(wc -c < $B/raven-ci) / 1024 )) KiB, raven-ci-agent $(( $(wc -c < $B/raven-ci-agent) / 1024 )) KiB"

kill $AGT; kill $SRV
sleep 3
kill -9 $AGT $SRV 2>/dev/null
wait $AGT $SRV 2>/dev/null
# Only the two it started: with PASTA_DOS_BINARIOS /usr/bin, a pattern on the path would also count
# the server of the RPM and its agents.
sobraram=0
for pid in $AGT $SRV; do kill -0 "$pid" 2>/dev/null && sobraram=$((sobraram + 1)); done
echo "processos que sobraram: $sobraram"
rm -rf -- "${S:?}"
