import {StrictMode,useState} from 'react';
import {createRoot} from 'react-dom/client';
import {Arena} from './Arena';
import {useFeed} from './useFeed';
import {label,score,tokenUrl,type Metric} from './data';
import './style.css';
function App(){
 const [demo,setDemo]=useState(new URLSearchParams(location.search).get('demo')==='1');
 const [metric,setMetric]=useState<Metric>('net_flow');const [paused,setPaused]=useState(false);const [list,setList]=useState(false);
 const {snapshot,connection}=useFeed(demo);
 const tokens=snapshot?.[metric].slice(0,50)||[];
 const synthetic=demo||snapshot?.status==='demo';
 const status=synthetic?'Demo':connection!=='Connected'?connection:snapshot?.status||'Connecting';
 const total=tokens.reduce((sum,t)=>sum+BigInt(metric==='net_flow'?t.net_lamports:t.buy_lamports),0n);
 const toggleDemo=()=>{const next=!demo;setDemo(next);const url=new URL(location.href);if(next)url.searchParams.set('demo','1');else url.searchParams.delete('demo');history.replaceState({},'',url);};
 return <main>
  <header><a className="brand" href="/" aria-label="BallPump home"><span className="brand-mark"><i/><i/></span>ballpump<span className="beta">beta</span></a><span className="tagline">Watch memecoins pump in real time.</span><div className="header-actions"><span className={`status ${status.toLowerCase()}`}><i/>{status}</span><button className="quiet" onClick={toggleDemo}>{demo?'Connect feed':'Explore demo'}</button></div></header>
  <section className="intro"><div><div className="eyebrow"><span className="tiny-orbit"/> Follow the buying pressure</div><h1>Small tokens.<br className="mobile-break"/> Big energy.</h1><p>Every ball is a token. More buying pressure, bigger ball.</p></div><div className="window-note"><span>Always moving</span><strong>10<span>min</span></strong><span>Continuously rolling window</span></div></section>
  <section className="arena-panel" aria-label="Token buying pressure">
   <div className="arena-toolbar"><div className="metric-tabs" role="group" aria-label="Ranking metric"><button aria-pressed={metric==='net_flow'} onClick={()=>setMetric('net_flow')}>10m Net Buy Flow</button><button aria-pressed={metric==='buy_volume'} onClick={()=>setMetric('buy_volume')}>10m Buy Volume</button></div><div className="arena-tools"><span className="unit">◎ SOL</span><button aria-pressed={paused} onClick={()=>setPaused(!paused)} aria-label={paused?'Resume animation':'Pause animation'}>{paused?'▶':'Ⅱ'}<span>{paused?'Resume':'Pause'}</span></button></div></div>
   <div className="arena-heading"><div><span className="legend-dot"/>{metric==='net_flow'?'Where the buys outweigh the sells':'Every buy, before subtracting sells'}</div><span>{tokens.length} balls <span className="dim">/ top 50</span></span></div>
   <Arena tokens={tokens} metric={metric} paused={paused} demo={!!synthetic}/>
   {!tokens.length&&<div className="empty"><span>◎</span><h2>{connection==='Connected'?'Waiting for the flow':'The arena is ready'}</h2><p>{connection==='Connected'?'Qualifying tokens will appear as trades arrive.':'Start the local feed, or explore the synthetic demo.'}</p>{!demo&&<button onClick={toggleDemo}>Explore demo</button>}</div>}
   <div className="arena-bottom"><span><span className="legend-dot"/> Ball size = {metric==='net_flow'?'positive net SOL flow':'gross SOL buy volume'}</span><span>{synthetic?'Synthetic activity. No real trades.':'Select a ball to open Pump.fun'} <span aria-hidden="true">↗</span></span></div>
  </section>
  <div className="under-arena"><p>{snapshot?.coverage||'Pump.fun + PumpSwap · SOL/WSOL quotes'}<span className="coverage-note">{synthetic?'Demo data':status==='live'?'Finalized activity · replay coverage unverified':status==='warming'?'Building the trailing window; coverage incomplete':status==='degraded'?'Decode or recovery gaps; totals may be incomplete':'Feed unavailable or behind; gaps are not zero volume'}</span></p><button className="quiet list-toggle" aria-expanded={list} onClick={()=>setList(!list)}>{list?'Hide':'Show'} token list <span>{list?'−':'+'}</span></button></div>
  <section className="flow-strip" aria-label="Activity summary"><div><span>{metric==='net_flow'?'Net flow in view':'Buy volume in view'}</span><strong>{(Number(total)/1e9).toLocaleString(undefined,{maximumFractionDigits:1})}<small> SOL</small></strong></div><div><span>Tokens indexed in window</span><strong>{snapshot?.indexed_tokens.toLocaleString()||'—'}</strong></div><div className="strip-explainer"><span className="mini-ball">↗</span><p>Buying pressure is a flow of funds.<br/>It doesn’t prove price growth or legitimacy.</p></div></section>
  {list&&<section className="token-list" aria-label="Token rankings"><div className="list-head"><h2>The current lineup</h2><span>Rolling 10 minutes · SOL</span></div>{tokens.map((t,i)=><div className="token-row" key={t.mint}><span className="rank">{i+1}</span><span className="token-avatar" style={{background:`hsl(${i*53%360} 40% 27%)`}}>{label(t).slice(0,1)}</span><div>{synthetic?<strong>{label(t)}</strong>:<a href={tokenUrl(t)} target="_blank" rel="noopener noreferrer">{label(t)} ↗</a>}<small>{t.mint.slice(0,6)}…{t.mint.slice(-6)}</small></div><span>{t.trade_count.toLocaleString()} trades</span><strong>{score(t,metric).toLocaleString(undefined,{maximumFractionDigits:2})} SOL</strong></div>)}</section>}
  <footer><span className="footer-brand">ballpump</span><span>Pump.fun + PumpSwap activity. A little more alive.</span><span>{snapshot?`Updated ${new Date(snapshot.as_of_ms).toLocaleTimeString()}`:'Awaiting feed'}</span></footer>
 </main>;
}
createRoot(document.getElementById('root')!).render(<StrictMode><App/></StrictMode>);
