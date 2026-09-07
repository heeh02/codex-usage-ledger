import { createElement } from 'react';
import { renderToStaticMarkup } from 'react-dom/server';
import { expect, it } from 'vitest';
import { I18nContext } from '../../i18n';
import { enMessages } from '../../locales/en';
import { zhCNMessages } from '../../locales/zh-CN';
import type { SessionDistributions as DistributionData } from '../../api/types';
import { SessionDistributions } from './SessionDistributions';

const usage={input:100,cached:80,cacheWrite:0,cacheWriteObservedInput:0,cacheWriteCoverage:0,uncached:20,output:20,reasoning:5,total:120};
const data:DistributionData={own:{models:[{id:'model-a',label:'model-a',events:1,usage}],accounts:[{id:null,label:'unknown',events:1,usage}]},
  tree:{models:[{id:'model-b',label:'model-b',events:2,usage:{...usage,input:200,cached:160,uncached:40,output:40,reasoning:10,total:240}}],accounts:null}};
function render(scope:'own'|'tree',messages:typeof enMessages|typeof zhCNMessages,localDistributions:DistributionData|null=data) {
  return renderToStaticMarkup(createElement(I18nContext.Provider,{value:{language:'en',setLanguage:()=>{},t:key=>messages[key]}},
    createElement(SessionDistributions,{scope,detail:{localDistributions}})));
}
it('changes visible distribution scope without deriving usage from labels or global totals',()=>{
  for(const messages of [zhCNMessages,enMessages]) {
    const own=render('own',messages);expect(own).toContain('model-a');expect(own).not.toContain('model-b');
    expect(own).toContain(messages['sessions.unknown_dimension']);expect(own).toContain('—');
    expect(own).toContain('role="region"');expect(own).toContain('tabindex="0"');
    const tree=render('tree',messages);expect(tree).toContain('model-b');expect(tree).not.toContain('model-a');
    expect(tree).toContain(messages['sessions.distributions_unavailable']);
  }
});
it('keeps absent API detail distinct from a recorded zero distribution',()=>{
  expect(render('own',enMessages,null)).toContain(enMessages['sessions.distributions_unavailable']);
  const zero={...usage,input:0,cached:0,uncached:0,output:0,reasoning:0,total:0};
  const observed:DistributionData={own:{models:[{id:'zero-model',label:'zero-model',events:1,usage:zero}],accounts:[]},tree:{models:null,accounts:null}};
  expect(render('own',enMessages,observed)).toContain('zero-model');
});
