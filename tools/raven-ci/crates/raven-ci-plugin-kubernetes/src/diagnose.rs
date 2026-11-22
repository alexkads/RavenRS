//! A stuck deploy in clear words: what the pods of a rollout say, and a refusal of the server.

use serde_json::Value;

fn text<'v>(value: &'v Value, path: &[&str]) -> &'v str {
    path.iter().try_fold(value, |value, key| value.get(*key)).and_then(Value::as_str).unwrap_or_default()
}

/// The label selector of a deployment, statefulset or daemonset, from its JSON
/// (`spec.selector.matchLabels`), as `-l` takes it: `app=site,tier=web`.
pub fn selector(resource: &Value) -> Option<String> {
    let labels = resource.get("spec")?.get("selector")?.get("matchLabels")?.as_object()?;
    let parts: Vec<String> =
        labels.iter().filter_map(|(k, v)| v.as_str().map(|v| format!("{k}={v}"))).collect();
    (!parts.is_empty()).then(|| parts.join(","))
}

/// The memory limit of the container `name` in the pod `pod`, as it is written: `256Mi`.
fn memory_limit(pod: &Value, name: &str) -> Option<String> {
    pod.get("spec")?
        .get("containers")?
        .as_array()?
        .iter()
        .find(|c| text(c, &["name"]) == name)
        .map(|c| text(c, &["resources", "limits", "memory"]).to_owned())
        .filter(|limit| !limit.is_empty())
}

/// What is wrong with the pods in `pods` (the JSON of `kubectl get pods -o json`), one message
/// per problem, in the order found, none twice.
pub fn pods(pods: &Value) -> Vec<String> {
    let mut found: Vec<String> = Vec::new();
    let mut say = |message: String| {
        if !found.contains(&message) {
            found.push(message);
        }
    };
    for pod in pods.get("items").and_then(Value::as_array).into_iter().flatten() {
        let name = text(pod, &["metadata", "name"]);
        let statuses = pod.get("status").and_then(|s| s.get("containerStatuses")).and_then(Value::as_array);
        for status in statuses.into_iter().flatten() {
            let container = text(status, &["name"]);
            let image = text(status, &["image"]);
            let waiting = text(status, &["state", "waiting", "reason"]);
            let detail = text(status, &["state", "waiting", "message"]);
            match waiting {
                "ImagePullBackOff" | "ErrImagePull" | "InvalidImageName" => {
                    let mut message = format!(
                        "a imagem {image} do pod {name} não pôde ser baixada ({waiting}): confira o nome, a tag e o acesso ao registro (imagePullSecrets)"
                    );
                    if !detail.is_empty() {
                        message.push_str(&format!(". O cluster diz: {detail}"));
                    }
                    say(message);
                }
                "CrashLoopBackOff" => say(format!(
                    "o contêiner {container} do pod {name} cai logo ao iniciar (CrashLoopBackOff): veja kubectl logs {name} -c {container} --previous"
                )),
                _ => {}
            }
            let killed = ["state", "lastState"]
                .iter()
                .any(|when| text(status, &[when, "terminated", "reason"]) == "OOMKilled");
            if killed {
                let limit = memory_limit(pod, container).map_or_else(
                    || "sem limite declarado".to_owned(),
                    |limit| format!("com o limite de {limit}"),
                );
                say(format!(
                    "o contêiner {container} do pod {name} foi morto por falta de memória (OOMKilled), {limit}: aumente resources.limits.memory"
                ));
            }
        }
        if text(pod, &["status", "phase"]) == "Pending" {
            let conditions = pod.get("status").and_then(|s| s.get("conditions")).and_then(Value::as_array);
            let unschedulable = conditions
                .into_iter()
                .flatten()
                .find(|c| text(c, &["type"]) == "PodScheduled" && text(c, &["status"]) == "False");
            if let Some(condition) = unschedulable {
                let why = text(condition, &["message"]);
                say(format!(
                    "sem recurso no cluster: o pod {name} está em Pending e nenhum nó o aceita{}",
                    if why.is_empty() { String::new() } else { format!(": {why}") }
                ));
            }
        }
    }
    found
}

/// A refusal of the server (`Error from server (Forbidden)`) in `lines`, explained: who cannot do
/// what, where.
pub fn forbidden(lines: &[String]) -> Option<String> {
    if !lines.iter().any(|l| l.contains("(Forbidden)") || l.contains(" is forbidden: ")) {
        return None;
    }
    // kubectl may say it in several lines ("error when retrieving…", then "… is forbidden: User
    // … cannot …"): read them as one, from where the reason is.
    let all = lines.iter().map(|l| l.trim()).collect::<Vec<_>>().join(" ");
    let line = all.find(" is forbidden: ").map_or(all.as_str(), |at| &all[at..]);
    let quoted = |after: &str| -> Option<String> {
        let rest = &line[line.find(after)? + after.len()..];
        let rest = rest.strip_prefix('"')?;
        Some(rest[..rest.find('"')?].to_owned())
    };
    let user = quoted("User ");
    let verb = line
        .find("cannot ")
        .map(|at| line[at + "cannot ".len()..].split_whitespace().next().unwrap_or_default().to_owned());
    let resource = quoted("resource ");
    let namespace = quoted("namespace ");
    let mut message = "o kubeconfig não tem permissão (403)".to_owned();
    if let (Some(user), Some(verb), Some(resource)) = (&user, &verb, &resource) {
        message.push_str(&format!(": o usuário {user} não pode {verb} {resource}"));
        match &namespace {
            Some(namespace) => message.push_str(&format!(" no namespace {namespace}")),
            None => message.push_str(" no cluster"),
        }
        message.push_str(". Dê a ele um Role (ou ClusterRole) com esse verbo, ou use outra credencial");
    } else {
        message.push_str(&format!(": {}", all.trim()));
    }
    Some(message)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn the_selector_of_a_deployment_is_its_match_labels() {
        let deployment =
            json!({ "spec": { "selector": { "matchLabels": { "app": "site", "tier": "web" } } } });
        assert_eq!(selector(&deployment).as_deref(), Some("app=site,tier=web"));
        assert_eq!(selector(&json!({})), None);
    }

    #[test]
    fn a_refusal_says_who_cannot_do_what_where() {
        let lines = vec![
            r#"Error from server (Forbidden): pods is forbidden: User "system:serviceaccount:ci:deployer" cannot create resource "pods" in API group "" in the namespace "prod""#.to_owned(),
        ];
        assert_eq!(
            forbidden(&lines).as_deref(),
            Some(
                "o kubeconfig não tem permissão (403): o usuário system:serviceaccount:ci:deployer não pode create pods no namespace prod. Dê a ele um Role (ou ClusterRole) com esse verbo, ou use outra credencial"
            )
        );
        assert_eq!(forbidden(&["error: something else".to_owned()]), None);
        let apply = [
            "Error from server (Forbidden): error when retrieving current configuration of:",
            "Resource: \"apps/v1, Resource=deployments\", GroupVersionKind: \"apps/v1, Kind=Deployment\"",
            "Name: \"site\", Namespace: \"t\"",
            "from server for: \"site.yml\": deployments.apps \"site\" is forbidden: User \"system:serviceaccount:t:leitor\" cannot get resource \"deployments\" in API group \"apps\" in the namespace \"t\"",
        ]
        .map(str::to_owned);
        assert!(
            forbidden(&apply)
                .expect("refused")
                .contains("o usuário system:serviceaccount:t:leitor não pode get deployments no namespace t"),
            "{:?}",
            forbidden(&apply)
        );
    }
}
