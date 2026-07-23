import { useEffect } from "react";
import { listen } from "@tauri-apps/api/event";

import { useAppStore } from "./store/appStore";

function App() {
  const status = useAppStore((s) => s.status);
  const tasks = useAppStore((s) => s.tasks);
  const error = useAppStore((s) => s.error);
  const loading = useAppStore((s) => s.loading);
  const refreshStatus = useAppStore((s) => s.refreshStatus);
  const refreshTasks = useAppStore((s) => s.refreshTasks);
  const runSmokeTask = useAppStore((s) => s.runSmokeTask);
  const upsertTask = useAppStore((s) => s.upsertTask);

  useEffect(() => {
    void refreshStatus();
    void refreshTasks();
    let unlisten: (() => void) | undefined;
    void listen("task-updated", (event) => {
      upsertTask(event.payload as Parameters<typeof upsertTask>[0]);
    }).then((fn) => {
      unlisten = fn;
    });
    return () => {
      unlisten?.();
    };
  }, [refreshStatus, refreshTasks, upsertTask]);

  return (
    <div className="flex h-full flex-col">
      <header className="flex items-center justify-between border-b border-white/10 px-6 py-4">
        <div>
          <p className="text-xs uppercase tracking-[0.2em] text-white/45">ScrapeX</p>
          <h1 className="text-xl font-semibold tracking-tight">跨平台骨架 · M0</h1>
        </div>
        <button
          type="button"
          onClick={() => void runSmokeTask()}
          className="rounded-md bg-sky-600 px-3 py-1.5 text-sm font-medium text-white hover:bg-sky-500"
        >
          跑 Smoke 任务
        </button>
      </header>

      <main className="grid flex-1 grid-cols-1 gap-4 overflow-auto p-6 lg:grid-cols-2">
        <section className="rounded-xl border border-white/10 bg-white/5 p-4">
          <h2 className="mb-3 text-sm font-semibold text-white/80">运行时状态</h2>
          {loading && !status ? (
            <p className="text-sm text-white/50">加载中…</p>
          ) : status ? (
            <dl className="space-y-2 text-sm">
              <Row label="版本" value={status.version} />
              <Row label="数据目录" value={status.dataDir} mono />
              <Row label="数据库" value={status.databasePath} mono />
              <Row label="资料库数" value={String(status.libraryCount)} />
              <Row
                label="Crates"
                value={`${status.crates.mediaCore} / ${status.crates.scraperKit} / ${status.crates.renamer}`}
              />
              <Row
                label="刮削并发"
                value={String(status.config.scrapeConcurrency)}
              />
              <Row label="元数据语言" value={status.config.metadataLanguage} />
              <Row label="NFO" value={status.config.nfoFormat} />
            </dl>
          ) : (
            <p className="text-sm text-white/50">尚无状态</p>
          )}
          {error ? <p className="mt-3 text-sm text-rose-400">{error}</p> : null}
        </section>

        <section className="rounded-xl border border-white/10 bg-white/5 p-4">
          <div className="mb-3 flex items-center justify-between">
            <h2 className="text-sm font-semibold text-white/80">任务队列</h2>
            <button
              type="button"
              onClick={() => void refreshTasks()}
              className="text-xs text-sky-400 hover:text-sky-300"
            >
              刷新
            </button>
          </div>
          {tasks.length === 0 ? (
            <p className="text-sm text-white/50">暂无任务。点右上角验证队列与事件。</p>
          ) : (
            <ul className="space-y-2">
              {tasks.map((task) => (
                <li
                  key={task.id}
                  className="rounded-lg border border-white/10 bg-black/20 px-3 py-2 text-sm"
                >
                  <div className="flex items-center justify-between gap-2">
                    <span className="font-medium">{task.title}</span>
                    <span className="text-xs uppercase tracking-wide text-white/50">
                      {task.status}
                    </span>
                  </div>
                  {task.progress ? (
                    <p className="mt-1 text-xs text-white/55">
                      {task.progress.current} ({task.progress.completed}/
                      {task.progress.total})
                    </p>
                  ) : null}
                  {task.errorMessage ? (
                    <p className="mt-1 text-xs text-rose-400">{task.errorMessage}</p>
                  ) : null}
                </li>
              ))}
            </ul>
          )}
        </section>
      </main>
    </div>
  );
}

function Row({
  label,
  value,
  mono,
}: {
  label: string;
  value: string;
  mono?: boolean;
}) {
  return (
    <div className="grid grid-cols-[7rem_1fr] gap-2">
      <dt className="text-white/45">{label}</dt>
      <dd className={mono ? "truncate font-mono text-xs text-white/85" : "text-white/85"}>
        {value}
      </dd>
    </div>
  );
}

export default App;
