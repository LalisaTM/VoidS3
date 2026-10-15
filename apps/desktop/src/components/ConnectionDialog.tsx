import { useState, type FormEvent } from "react";
import { useQueryClient } from "@tanstack/react-query";
import { Check, FlaskConical, ShieldCheck } from "lucide-react";
import { api, message, type Profile } from "../lib/api";
import { Modal } from "./Modal";
import { Button } from "./ui/button";
const presets: Record<
  string,
  { label: string; endpoint: string; region: string; path_style: boolean }
> = {
  r2: {
    label: "Cloudflare R2",
    endpoint: "",
    region: "auto",
    path_style: true,
  },
  aws: {
    label: "Amazon S3",
    endpoint: "https://s3.us-east-1.amazonaws.com",
    region: "us-east-1",
    path_style: false,
  },
  b2: {
    label: "Backblaze B2",
    endpoint: "https://s3.us-west-004.backblazeb2.com",
    region: "us-west-004",
    path_style: true,
  },
  wasabi: {
    label: "Wasabi",
    endpoint: "https://s3.wasabisys.com",
    region: "us-east-1",
    path_style: true,
  },
  minio: {
    label: "MinIO",
    endpoint: "http://127.0.0.1:9000",
    region: "us-east-1",
    path_style: true,
  },
  spaces: {
    label: "DigitalOcean Spaces",
    endpoint: "https://nyc3.digitaloceanspaces.com",
    region: "nyc3",
    path_style: false,
  },
  custom: {
    label: "Custom S3",
    endpoint: "https://",
    region: "us-east-1",
    path_style: true,
  },
};
export function ConnectionDialog({
  initial,
  onClose,
  onSaved,
}: {
  initial?: Profile;
  onClose: () => void;
  onSaved: (profile: Profile) => void;
}) {
  const [profile, setProfile] = useState<Profile>(
    () =>
      initial ?? {
        id: crypto.randomUUID(),
        name: "",
        provider: "r2",
        endpoint: "",
        region: "auto",
        bucket: "",
        prefix: "",
        path_style: true,
        read_only: false,
        allow_http: false,
      },
  );
  const [access, setAccess] = useState("");
  const [secret, setSecret] = useState("");
  const [token, setToken] = useState("");
  const [account, setAccount] = useState("");
  const [jurisdiction, setJurisdiction] = useState("");
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState("");
  const [tested, setTested] = useState(false);
  const [saved, setSaved] = useState(!!initial);
  const client = useQueryClient();
  const patch = (value: Partial<Profile>) => {
    setProfile((p) => ({ ...p, ...value }));
    setTested(false);
  };
  async function save(test: boolean) {
    setBusy(true);
    setError("");
    try {
      const normalized = {
        ...profile,
        prefix:
          profile.prefix && !profile.prefix.endsWith("/")
            ? `${profile.prefix}/`
            : profile.prefix,
      };
      await api("save_profile", {
        profile: normalized,
        credentials:
          access || secret
            ? {
                access_key: access,
                secret_key: secret,
                session_token: token || null,
              }
            : null,
      });
      setSaved(true);
      await client.invalidateQueries({ queryKey: ["profiles"] });
      if (test) {
        await api("test_connection", { id: profile.id });
        setTested(true);
      } else {
        onSaved(normalized);
        onClose();
      }
      setAccess("");
      setSecret("");
      setToken("");
    } catch (e) {
      setError(message(e));
    } finally {
      setBusy(false);
    }
  }
  function submit(e: FormEvent) {
    e.preventDefault();
    void save(false);
  }
  function r2Endpoint(id: string, area: string) {
    setAccount(id);
    setJurisdiction(area);
    patch({
      endpoint: id
        ? `https://${id}${area ? `.${area}` : ""}.r2.cloudflarestorage.com`
        : "",
    });
  }
  return (
    <Modal
      title={initial ? "Edit connection" : "New connection"}
      description="Connect directly to a bucket. Account-wide bucket access is never required."
      onClose={onClose}
    >
      <form onSubmit={submit} className="form">
        <div className="form-grid">
          <label>
            Connection name
            <input
              autoFocus
              required
              value={profile.name}
              onChange={(e) => patch({ name: e.target.value })}
              placeholder="Production assets"
            />
          </label>
          <label>
            Provider
            <select
              value={profile.provider}
              onChange={(e) =>
                patch({
                  provider: e.target.value,
                  ...presets[e.target.value],
                  allow_http: e.target.value === "minio",
                })
              }
            >
              {Object.entries(presets).map(([key, p]) => (
                <option key={key} value={key}>
                  {p.label}
                </option>
              ))}
            </select>
          </label>
        </div>
        {profile.provider === "r2" && (
          <div className="form-grid">
            <label>
              R2 account ID <span className="muted">(endpoint helper)</span>
              <input
                value={account}
                onChange={(e) => r2Endpoint(e.target.value, jurisdiction)}
                placeholder="Cloudflare account ID"
              />
            </label>
            <label>
              Jurisdiction
              <select
                value={jurisdiction}
                onChange={(e) => r2Endpoint(account, e.target.value)}
              >
                <option value="">Default</option>
                <option value="eu">European Union</option>
                <option value="fedramp">FedRAMP</option>
              </select>
            </label>
          </div>
        )}
        <label>
          Endpoint URL
          <input
            type="url"
            required
            value={profile.endpoint}
            onChange={(e) => patch({ endpoint: e.target.value })}
            placeholder="https://account-id.r2.cloudflarestorage.com"
          />
        </label>
        <div className="form-grid">
          <label>
            Bucket
            <input
              required
              value={profile.bucket}
              onChange={(e) => patch({ bucket: e.target.value })}
              placeholder="my-bucket"
            />
          </label>
          <label>
            Signing region
            <input
              required
              value={profile.region}
              onChange={(e) => patch({ region: e.target.value })}
            />
          </label>
        </div>
        <label>
          Initial prefix
          <input
            value={profile.prefix}
            onChange={(e) => patch({ prefix: e.target.value })}
            placeholder="Optional, e.g. assets/"
          />
        </label>
        <div className="section-label">
          <ShieldCheck size={14} /> OS KEYCHAIN
        </div>
        <div className="form-grid">
          <label>
            Access key ID
            <input
              required={!saved}
              autoComplete="off"
              value={access}
              onChange={(e) => setAccess(e.target.value)}
              placeholder={
                saved ? "Leave blank to keep credentials" : "Access key ID"
              }
            />
          </label>
          <label>
            Secret access key
            <input
              type="password"
              required={!saved}
              autoComplete="new-password"
              value={secret}
              onChange={(e) => setSecret(e.target.value)}
            />
          </label>
        </div>
        <label>
          Session token <span className="muted">(optional)</span>
          <input
            type="password"
            autoComplete="off"
            value={token}
            onChange={(e) => setToken(e.target.value)}
          />
        </label>
        <div className="check-row">
          <label>
            <input
              type="checkbox"
              checked={profile.path_style}
              onChange={(e) => patch({ path_style: e.target.checked })}
            />{" "}
            Path-style addressing
          </label>
          <label>
            <input
              type="checkbox"
              checked={profile.read_only}
              onChange={(e) => patch({ read_only: e.target.checked })}
            />{" "}
            Read-only
          </label>
        </div>
        <label className="checkbox">
          <input
            type="checkbox"
            checked={profile.allow_http}
            onChange={(e) => patch({ allow_http: e.target.checked })}
          />{" "}
          Allow unencrypted HTTP (trusted local test services only)
        </label>
        {profile.allow_http && (
          <p className="warning">
            HTTP exposes credentials and transferred data to the network.
          </p>
        )}
        {error && (
          <p role="alert" className="error-box">
            {error}
          </p>
        )}
        {tested && (
          <p className="success">
            <Check size={14} /> Bucket access verified.
          </p>
        )}
        <div className="modal-footer">
          <Button disabled={busy} onClick={() => void save(true)}>
            <FlaskConical size={15} /> Save & test
          </Button>
          <Button type="submit" variant="default" disabled={busy}>
            {busy ? "Working…" : "Save connection"}
          </Button>
        </div>
      </form>
    </Modal>
  );
}
