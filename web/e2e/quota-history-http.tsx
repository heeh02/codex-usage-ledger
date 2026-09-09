import { createRoot } from 'react-dom/client';
import { QuotaHistoryPanel } from '../src/features/accounts/QuotaHistoryPanel';
import { I18nProvider } from '../src/I18nProvider';
import '../src/styles.css';
createRoot(document.getElementById('root')!).render(<I18nProvider><main style={{ padding: 16, height: '100vh', overflow: 'auto' }}>
  <p>SYNTHETIC COMPONENT QA — HTTP responses supplied by the test</p>
  <QuotaHistoryPanel account="all" demo={false} timezone="UTC" accountName={id => id}/>
</main></I18nProvider>);
