import { createRoot } from 'react-dom/client';
import { useState } from 'react';
import { I18nProvider } from '../src/I18nProvider';
import { useI18n } from '../src/i18n';
import { SessionDistributions } from '../src/features/sessions/SessionDistributions';
import type { SessionDistributions as Data } from '../src/api/types';
import '../src/styles.css';

const usage={input:100,cached:70,cacheWrite:10,cacheWriteObservedInput:100,cacheWriteCoverage:1,uncached:20,output:20,reasoning:5,total:120};
const data:Data={own:{models:[{id:'model-a',label:'Model A',events:1,usage}],accounts:[{id:null,label:'unknown',events:1,usage}]},
tree:{models:[{id:'model-a',label:'Model A',events:1,usage},{id:'model-b',label:'Model B',events:1,usage}],accounts:[{id:'account-a',label:'Account A',events:1,usage},{id:null,label:'unknown',events:1,usage}]}};
function Fixture(){const {language,setLanguage}=useI18n();const [scope,setScope]=useState<'own'|'tree'>('own');const [width,setWidth]=useState(900);return <main style={{width:'100%',height:'100vh',overflow:'auto',padding:16,boxSizing:'border-box'}}>
<p>SYNTHETIC COMPONENT QA · NOT REAL USAGE</p><button onClick={()=>setLanguage(language==='en'?'zh-CN':'en')}>Language</button>
<button onClick={()=>setScope(scope==='own'?'tree':'own')}>Own / Tree</button><button onClick={()=>setWidth(width===900?360:900)}>Narrow / Wide</button>
<div style={{width,maxWidth:'100%'}}><SessionDistributions detail={{localDistributions:data}} scope={scope}/></div></main>}
createRoot(document.getElementById('root')!).render(<I18nProvider><Fixture/></I18nProvider>);
