"use client";

import React, { useState, useEffect, useRef } from "react";
import ReactMarkdown from "react-markdown";
import { Button } from "@/components/ui/button";
import { ChevronLeft, ChevronRight } from "lucide-react";
import {
  Dialog,
  DialogContent,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog";

export function AssistantPanel({ videoId, rawJson }: { videoId: string; rawJson: any }) {
  const [enrichedReport, setEnrichedReport] = useState<string | null>(null);
  const [loading, setLoading] = useState(false);
  
  // Document state for Dialogs
  const [documentContent, setDocumentContent] = useState<{ kind: string, text?: string, data?: any } | null>(null);
  const [dialogOpen, setDialogOpen] = useState(false);
  const [docLoading, setDocLoading] = useState(false);

  // LLM debug payload and response
  const [llmRequest, setLlmRequest] = useState<any>(null);
  const [llmResponse, setLlmResponse] = useState<any>(null);

  const [toolsString, setToolsString] = useState<string | null>(null);
  const hasFetchedRef = useRef(false);

  useEffect(() => {
    fetch("/api/tools")
      .then(res => res.json())
      .then(body => {
        const str = body.tools.map((t: any) => `- ${t.name}: ${t.description}`).join("\n");
        setToolsString(str);
      })
      .catch(err => console.error("Tools fetch error:", err));
  }, []);

  // Otomatik üretim tetikleyicisi
  useEffect(() => {
    if (rawJson && toolsString !== null && !enrichedReport && !loading && !hasFetchedRef.current) {
      hasFetchedRef.current = true;
      handleEnhance();
    }
  }, [rawJson, toolsString, enrichedReport, loading]);

  const handleEnhance = async () => {
    setLoading(true);
    
    const requestBody = {
      report_json: JSON.stringify(rawJson),
      tools: toolsString
    };
    
    setLlmRequest(requestBody);

    try {
      const res = await fetch("/api/humanizer/v1/humanize", {
        method: "POST",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify(requestBody)
      });

      if (!res.ok) throw new Error("Enhance request failed");
      const data = await res.json();
      
      // Update llmRequest to show the actual prompt that went to the LLM
      if (data.prompt) {
        setLlmRequest({
          ...requestBody,
          actual_generated_prompt: data.prompt
        });
      }
      
      setLlmResponse(data);
      setEnrichedReport(data.result);
    } catch (err) {
      console.error("Enhance error:", err);
      setLlmResponse({ error: String(err) });
    } finally {
      setLoading(false);
    }
  };

  const handleGenerateDocument = async (kind: "dilekce" | "tutanak") => {
    setDocLoading(true);
    setDialogOpen(true);
    setDocumentContent(null);
    try {
      const res = await fetch("/api/humanizer/v1/document", {
        method: "POST",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify({
          kind,
          report_json: JSON.stringify(rawJson),
        })
      });

      if (!res.ok) throw new Error("Document request failed");
      const data = await res.json();
      setDocumentContent({ kind, text: data.result });
    } catch (err) {
      console.error("Document error:", err);
      setDocumentContent({ kind, text: "Bir hata oluştu. Lütfen tekrar deneyin." });
    } finally {
      setDocLoading(false);
    }
  };

  const handlePrint = () => {
    if (!documentContent || !documentContent.text) return;
    const win = window.open("", "_blank");
    if (!win) return;
    win.document.write(`
      <html>
        <head>
          <title>${documentContent.kind.toUpperCase()}</title>
          <style>
            body { font-family: sans-serif; line-height: 1.6; padding: 40px; max-width: 800px; margin: 0 auto; }
            pre { white-space: pre-wrap; font-family: inherit; }
          </style>
        </head>
        <body>
          <pre>${documentContent.text}</pre>
          <script>window.print();</script>
        </body>
      </html>
    `);
    win.document.close();
  };

  return (
    <div className="flex flex-col space-y-4 text-sm w-full pt-2">
      {/* Araç Çağrıları */}
      <div className="mb-2">
        <ToolCarousel 
          videoId={videoId} 
          actions={llmResponse?.tool_calls?.map((t: any) => t.name) || []} 
          running={loading}
          autoApprove={false} 
        />
      </div>

      {/* Asistan Anlatımı Bubble */}
      <div>
        <h3 className="font-semibold text-lg mb-2 px-1 text-stone-900 dark:text-stone-100">Asistan Anlatımı</h3>
        {loading && !enrichedReport && (
          <div className="flex items-center gap-2 text-stone-500 p-4 bg-stone-100 dark:bg-stone-900 rounded-2xl w-fit">
            <span className="animate-pulse">Düşünüyor...</span>
          </div>
        )}
        
        {enrichedReport && (
          <div className="bg-stone-100 dark:bg-stone-800/80 rounded-2xl rounded-tl-sm p-4 text-stone-800 dark:text-stone-200 max-h-[400px] overflow-y-auto scrollbar-thin scrollbar-thumb-stone-300 dark:scrollbar-thumb-stone-600 shadow-sm prose prose-sm dark:prose-invert prose-stone">
            <ReactMarkdown>{enrichedReport}</ReactMarkdown>
          </div>
        )}
      </div>

      {/* Butonlar */}
      <div className="flex gap-3 pt-1">
        <Button onClick={() => handleGenerateDocument("dilekce")} disabled={loading} variant="default" className="flex-1 font-semibold rounded-xl">
          Dilekçe
        </Button>
        <Button onClick={() => handleGenerateDocument("tutanak")} disabled={loading} variant="default" className="flex-1 font-semibold rounded-xl">
          Rapor
        </Button>
      </div>

      <div className="flex gap-3 pb-2 pt-1">
        <Button onClick={() => { setDocumentContent({ kind: 'Gelen Veri (LLM)', data: llmResponse || { message: "Henüz yanıt yok." } }); setDialogOpen(true); }} variant="outline" className="flex-1 text-xs">
          Gelen Veri
        </Button>
        <Button onClick={() => { setDocumentContent({ kind: 'Giden Veri (LLM)', data: llmRequest || { message: "Henüz istek yapılmadı." } }); setDialogOpen(true); }} variant="outline" className="flex-1 text-xs">
          Giden Veri
        </Button>
      </div>

      {/* Dialog Gösterimi */}
      <Dialog open={dialogOpen} onOpenChange={setDialogOpen}>
        <DialogContent className="max-w-[90vw] lg:max-w-[1200px] w-full max-h-[90vh] flex flex-col">
          <DialogHeader className="shrink-0 flex flex-row items-center justify-between">
            <DialogTitle className="capitalize text-xl">
              {documentContent?.kind === "tutanak" ? "Rapor (Tutanak)" : documentContent?.kind || "Belge"}
            </DialogTitle>
            {documentContent?.text && (
              <Button size="sm" onClick={handlePrint} variant="outline" disabled={docLoading} className="mr-8">
                Yazdır / PDF
              </Button>
            )}
          </DialogHeader>
          
          <div className="flex-1 overflow-y-auto mt-2 p-1 scrollbar-thin">
            {docLoading ? (
              <div className="flex items-center justify-center h-40 text-stone-500 animate-pulse">
                Belge oluşturuluyor, lütfen bekleyin...
              </div>
            ) : documentContent ? (
              <div className="text-sm whitespace-pre-wrap font-mono bg-stone-50 dark:bg-stone-950 p-6 rounded-lg border border-stone-200 dark:border-stone-800 text-stone-800 dark:text-stone-200 shadow-inner">
                {documentContent.data ? <JsonViewer data={documentContent.data} /> : documentContent.text}
              </div>
            ) : null}
          </div>
        </DialogContent>
      </Dialog>
    </div>
  );
}

function JsonViewer({ data, name = "root" }: { data: any, name?: string }) {
  const [collapsed, setCollapsed] = React.useState(false);
  const isObject = data !== null && typeof data === 'object';
  const isArray = Array.isArray(data);
  const keys = isObject ? Object.keys(data) : [];
  const isEmpty = isObject && keys.length === 0;

  if (!isObject) {
    const isString = typeof data === 'string';
    const color = isString ? 'text-green-600 dark:text-green-400' : typeof data === 'number' ? 'text-blue-600 dark:text-blue-400' : typeof data === 'boolean' ? 'text-purple-600 dark:text-purple-400' : 'text-gray-500';
    return <span className={color}>{isString ? `"${data}"` : String(data)}</span>;
  }

  if (isEmpty) {
    return <span>{isArray ? '[]' : '{}'}</span>;
  }

  return (
    <div className="font-mono text-[13px] ml-4 border-l border-stone-200 dark:border-stone-700 pl-2">
      <div 
        className="cursor-pointer hover:bg-stone-100 dark:hover:bg-stone-800 rounded px-1 -ml-3 inline-flex items-center select-none"
        onClick={() => setCollapsed(!collapsed)}
      >
        <span className="w-4 inline-block text-center text-stone-400 font-bold">{collapsed ? '+' : '-'}</span>
        <span className="text-stone-500 dark:text-stone-400">{isArray ? '[' : '{'}</span>
        {collapsed && <span className="text-stone-500 dark:text-stone-400 px-2">... {isArray ? ']' : '}'}</span>}
      </div>
      
      {!collapsed && (
        <div className="ml-4">
          {keys.map((key, i) => {
            const val = data[key];
            let rawStr = typeof val === 'string' && val.startsWith('{') ? val : null;
            let parsedVal = val;
            if (rawStr) {
              try {
                parsedVal = JSON.parse(rawStr);
              } catch (e) {}
            }
            return (
              <div key={key} className="py-0.5">
                {!isArray && <span className="text-pink-600 dark:text-pink-400 font-medium">"{key}"</span>}
                {!isArray && <span className="mr-2">:</span>}
                <JsonViewer data={parsedVal} name={key} />
                {i < keys.length - 1 && <span>,</span>}
              </div>
            );
          })}
        </div>
      )}
      {!collapsed && <div className="text-stone-500 dark:text-stone-400">{isArray ? ']' : '}'}</div>}
    </div>
  );
}
function ActionExecuteButton({ videoId, action, autoApprove }: { videoId: string, action: string, autoApprove?: boolean }) {
  const [status, setStatus] = React.useState<"idle" | "running" | "success" | "error">("idle")
  const executedRef = React.useRef(false)

  const handleExecute = async () => {
    if (executedRef.current) return
    executedRef.current = true
    setStatus("running")
    try {
      const r = await fetch("/api/tools/execute", {
        method: "POST",
        headers: { "content-type": "application/json" },
        body: JSON.stringify({
          video_id: videoId,
          tool_name: action,
          payload: {}
        })
      })
      if (!r.ok) throw new Error("Hata")
      setStatus("success")
    } catch (e) {
      setStatus("error")
      setTimeout(() => {
        setStatus("idle")
        executedRef.current = false
      }, 2000)
    }
  }

  // Sadece ilgili action değişirse executedRef sıfırlansın
  React.useEffect(() => {
    executedRef.current = false;
    setStatus("idle");
  }, [action])

  React.useEffect(() => {
    if (autoApprove && status === "idle" && !executedRef.current) {
      handleExecute()
    }
  }, [autoApprove, status, action])

  return (
    <div className="flex gap-2 mt-4">
      <Button 
        size="sm" 
        variant={status === "success" ? "secondary" : "default"} 
        disabled={status !== "idle"}
        onClick={handleExecute}
        className="flex-1 font-medium"
      >
        {status === "idle" && "Kabul Et"}
        {status === "running" && "Çalıştırılıyor..."}
        {status === "success" && "Çalıştırıldı"}
        {status === "error" && "Hata!"}
      </Button>
      {status === "idle" && (
         <Button size="sm" variant="outline" className="flex-1 text-muted-foreground hover:text-destructive hover:bg-destructive/10">
           Reddet
         </Button>
      )}
    </div>
  )
}

function ToolCarousel({ videoId, actions, autoApprove, running }: { videoId: string, actions: string[], autoApprove?: boolean, running: boolean }) {
  const [tools, setTools] = React.useState<any[]>([])
  const [activeIndex, setActiveIndex] = React.useState(0)

  React.useEffect(() => {
    fetch("/api/tools").then(r => r.json()).then(data => setTools(data.tools || [])).catch(console.error)
  }, [])

  React.useEffect(() => {
    if (actions.length > 0) {
      setActiveIndex(actions.length - 1)
    }
  }, [actions])

  if (running) {
    return (
      <div className="flex shrink-0 flex-col gap-2.5 rounded-xl border bg-card px-5 py-6 animate-pulse">
        <div className="h-4 bg-muted rounded w-1/3 mb-3"></div>
        <div className="h-3 bg-muted rounded w-3/4"></div>
        <div className="h-3 bg-muted rounded w-1/2 mt-2"></div>
        <div className="flex gap-2 mt-6">
          <div className="h-8 bg-muted rounded flex-1"></div>
          <div className="h-8 bg-muted rounded flex-1"></div>
        </div>
      </div>
    )
  }

  if (actions.length === 0) {
    return (
      <div className="flex shrink-0 items-center justify-center rounded-xl border bg-card px-4 py-8 text-muted-foreground border-dashed">
        <p className="text-sm">Henüz bir araç çağrısı yapılmadı.</p>
      </div>
    )
  }

  const currentAction = actions[activeIndex]
  const toolInfo = tools.find(t => t.name === currentAction) || { title: currentAction, description: "Araç detayı bulunamadı." }

  return (
    <div className="flex shrink-0 flex-col rounded-xl border bg-card px-5 py-5 relative overflow-hidden shadow-sm">
      <div className="flex items-center justify-between mb-3 border-b pb-2">
        <span className="text-xs font-semibold uppercase tracking-wider text-muted-foreground">Araç Çağrısı</span>
        <div className="flex gap-2 items-center">
          <Button size="icon" variant="ghost" className="h-6 w-6 rounded-full hover:bg-muted" disabled={activeIndex === 0} onClick={() => setActiveIndex(activeIndex - 1)}>
            <ChevronLeft className="h-4 w-4" />
          </Button>
          <span className="text-[11px] font-mono text-muted-foreground font-medium">{activeIndex + 1} / {actions.length}</span>
          <Button size="icon" variant="ghost" className="h-6 w-6 rounded-full hover:bg-muted" disabled={activeIndex === actions.length - 1} onClick={() => setActiveIndex(activeIndex + 1)}>
            <ChevronRight className="h-4 w-4" />
          </Button>
        </div>
      </div>
      
      <div className="flex flex-col gap-1.5 min-h-[60px]">
        <h3 className="font-bold text-base text-card-foreground">{toolInfo.title}</h3>
        <p className="text-sm text-muted-foreground line-clamp-3 leading-relaxed">{toolInfo.description}</p>
      </div>

      <ActionExecuteButton videoId={videoId} action={currentAction} autoApprove={autoApprove && activeIndex === actions.length - 1} />
    </div>
  )
}
