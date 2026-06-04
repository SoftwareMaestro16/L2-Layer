const state = {
  apiBase: "http://127.0.0.1:8080",
  adminToken: "",
  registryUrl: "../deployments/testnet/entropis.json",
};

const $ = (id) => document.getElementById(id);

function apiUrl(path) {
  return `${state.apiBase.replace(/\/+$/, "")}${path}`;
}

async function request(path, options = {}) {
  const headers = { accept: "application/json" };
  if (options.admin) {
    if (!state.adminToken) {
      throw new Error("admin token required");
    }
    headers.authorization = `Bearer ${state.adminToken}`;
  }
  const response = await fetch(apiUrl(path), { headers });
  const text = await response.text();
  if (!response.ok) {
    throw new Error(`HTTP ${response.status}: ${safeError(text)}`);
  }
  return text ? JSON.parse(text) : null;
}

function safeError(text) {
  try {
    const parsed = JSON.parse(text);
    if (typeof parsed.error === "string") {
      return parsed.error;
    }
  } catch {
    // Keep proxy text as plain text; rendering uses textContent.
  }
  return text || "request failed";
}

function setJson(id, value) {
  $(id).textContent = typeof value === "string" ? value : JSON.stringify(value, null, 2);
}

function clear(node) {
  while (node.firstChild) {
    node.removeChild(node.firstChild);
  }
}

function text(value) {
  if (value === null || value === undefined || value === "") {
    return "-";
  }
  return String(value);
}

function shortHash(value) {
  const rendered = text(value);
  if (rendered.length <= 18) {
    return rendered;
  }
  return `${rendered.slice(0, 10)}...${rendered.slice(-8)}`;
}

function pill(value) {
  const span = document.createElement("span");
  const rendered = text(value);
  span.className = `pill ${pillTone(rendered)}`;
  span.textContent = rendered;
  return span;
}

function pillTone(value) {
  if (/failed|error|unavailable|not_ready/i.test(value)) {
    return "bad";
  }
  if (/pending|submitted|waiting|unknown/i.test(value)) {
    return "warn";
  }
  return "ok";
}

function renderMetric(container, label, value) {
  const item = document.createElement("div");
  item.className = "metric";
  const span = document.createElement("span");
  span.textContent = label;
  const strong = document.createElement("strong");
  if (value instanceof Node) {
    strong.appendChild(value);
  } else {
    strong.textContent = text(value);
  }
  item.append(span, strong);
  container.appendChild(item);
}

function renderSummary(summary) {
  const root = $("summary");
  clear(root);
  renderMetric(root, "latest block", summary.latest_block?.height);
  renderMetric(root, "block hash", shortHash(summary.latest_block?.block_hash));
  renderMetric(root, "batch commit", pill(summary.latest_batch_commit?.status));
  renderMetric(root, "confirmed batch", summary.latest_confirmed_commit?.batch_no);
  renderMetric(root, "finalization", pill(summary.latest_finalization?.status));
  renderMetric(root, "finalized batch", summary.latest_finalized_batch?.batch_no);
}

function renderTable(root, columns, rows) {
  clear(root);
  const table = document.createElement("table");
  const thead = document.createElement("thead");
  const headRow = document.createElement("tr");
  for (const column of columns) {
    const th = document.createElement("th");
    th.textContent = column.label;
    headRow.appendChild(th);
  }
  thead.appendChild(headRow);
  const tbody = document.createElement("tbody");
  for (const row of rows) {
    const tr = document.createElement("tr");
    for (const column of columns) {
      const td = document.createElement("td");
      const value = column.value(row);
      if (value instanceof Node) {
        td.appendChild(value);
      } else {
        td.textContent = text(value);
      }
      tr.appendChild(td);
    }
    tbody.appendChild(tr);
  }
  table.append(thead, tbody);
  root.appendChild(table);
}

function renderBlocks(blocks) {
  renderTable($("blocks"), [
    { label: "Height", value: (row) => row.height },
    { label: "Hash", value: (row) => shortHash(row.block_hash) },
    { label: "Txs", value: (row) => row.tx_count },
    { label: "Deposits", value: (row) => row.deposit_count },
    { label: "Withdrawals", value: (row) => row.withdrawal_count },
    { label: "State root", value: (row) => shortHash(row.state_root) },
  ], blocks.items ?? []);
}

function renderDeposits(deposits) {
  renderTable($("deposits"), [
    { label: "Status", value: (row) => pill(row.status) },
    { label: "Block", value: (row) => row.block_height },
    { label: "Deposit id", value: (row) => shortHash(row.deposit?.deposit_id) },
    { label: "Recipient", value: (row) => shortHash(row.deposit?.recipient) },
    { label: "Asset", value: (row) => row.deposit?.asset_id },
    { label: "Amount", value: (row) => row.deposit?.amount },
  ], deposits.items ?? []);
}

async function refreshPublic() {
  try {
    const [summary, blocks, deposits] = await Promise.all([
      request("/v1/explorer/summary"),
      request("/v1/explorer/blocks?limit=12"),
      request("/v1/explorer/deposits?limit=12"),
    ]);
    renderSummary(summary);
    renderBlocks(blocks);
    renderDeposits(deposits);
  } catch (error) {
    setJson("lookup-result", error.message);
  }
}

async function refreshOperator() {
  try {
    const [ready, failures, relayer, finalizer] = await Promise.all([
      request("/readyz"),
      request("/v1/operator/failures", { admin: true }),
      request("/v1/operator/batch-relayer", { admin: true }),
      request("/v1/operator/batch-finalizer", { admin: true }),
    ]);
    const status = $("operator-status");
    clear(status);
    renderMetric(status, "readiness", pill(ready.status));
    renderMetric(status, "failed relays", failures.relayer_failed_batches?.length ?? 0);
    renderMetric(status, "failed finalizations", failures.failed_finalizations?.length ?? 0);
    setJson("operator-json", { ready, failures, relayer, finalizer });
  } catch (error) {
    setJson("operator-json", error.message);
  }
}

async function lookup(kind) {
  const map = {
    tx: ["tx-hash", (value) => `/v1/tx/${encodeURIComponent(value)}`],
    account: ["account-id", (value) => `/v1/account/${encodeURIComponent(value)}`],
    deposit: ["deposit-id", (value) => `/v1/explorer/deposit/${encodeURIComponent(value)}`],
    withdrawal: ["withdrawal-id", (value) => `/v1/explorer/withdrawal/${encodeURIComponent(value)}`],
  };
  const [inputId, path] = map[kind];
  const value = $(inputId).value.trim();
  if (!value) {
    setJson("lookup-result", `${kind} value required`);
    return;
  }
  try {
    setJson("lookup-result", await request(path(value)));
  } catch (error) {
    setJson("lookup-result", error.message);
  }
}

async function loadRegistry() {
  const root = $("contracts");
  clear(root);
  try {
    const response = await fetch(state.registryUrl);
    if (!response.ok) {
      throw new Error(`registry HTTP ${response.status}`);
    }
    const registry = await response.json();
    const rollupRoot = findAddress(registry, ["rollupRoot", "rollup_root", "rollupRootAddress"]);
    const assetVault = findAddress(registry, ["assetVault", "asset_vault", "assetVaultAddress"]);
    renderContract(root, "RollupRoot", rollupRoot);
    renderContract(root, "AssetVault", assetVault);
  } catch (error) {
    renderMetric(root, "registry", error.message);
  }
}

function findAddress(value, keys) {
  if (!value || typeof value !== "object") {
    return null;
  }
  for (const [key, child] of Object.entries(value)) {
    if (typeof child === "string" && keys.some((needle) => key.toLowerCase().includes(needle.toLowerCase()))) {
      return child;
    }
    const nested = findAddress(child, keys);
    if (nested) {
      return nested;
    }
  }
  return null;
}

function renderContract(root, label, address) {
  if (!address) {
    renderMetric(root, label, "not configured");
    return;
  }
  const link = document.createElement("a");
  link.href = `https://testnet.tonviewer.com/${encodeURIComponent(address)}`;
  link.target = "_blank";
  link.rel = "noreferrer";
  link.textContent = shortHash(address);
  renderMetric(root, label, link);
}

function readSettings() {
  state.apiBase = $("api-base").value.trim() || state.apiBase;
  state.adminToken = $("admin-token").value;
  state.registryUrl = $("registry-url").value.trim() || state.registryUrl;
}

function bind() {
  $("settings-form").addEventListener("submit", (event) => {
    event.preventDefault();
    readSettings();
    refreshPublic();
  });
  $("refresh-public").addEventListener("click", () => {
    readSettings();
    refreshPublic();
  });
  $("refresh-operator").addEventListener("click", () => {
    readSettings();
    refreshOperator();
  });
  $("load-registry").addEventListener("click", () => {
    readSettings();
    loadRegistry();
  });
  $("lookup-tx").addEventListener("click", () => lookup("tx"));
  $("lookup-account").addEventListener("click", () => lookup("account"));
  $("lookup-deposit").addEventListener("click", () => lookup("deposit"));
  $("lookup-withdrawal").addEventListener("click", () => lookup("withdrawal"));
}

bind();
refreshPublic();
loadRegistry();
