import { useState } from 'react';
import { 
  Receipt, 
  Car, 
  Calculator, 
  CreditCard 
} from 'lucide-react';

interface Claim {
  id: string;
  employee_id: string;
  employee_name: string;
  category: string;
  amount_cents: number;
  net_cents: number;
  tax_cents: number;
  tax_key: string;
  is_kleinbetragsrechnung: boolean;
  status: 'submitted' | 'approved' | 'reimbursed' | 'rejected';
  claim_date: string;
  notes?: string;
}

interface Trip {
  id: string;
  origin: string;
  destination: string;
  distance_km: number;
  vehicle: string;
  passengers: number;
  allowance_cents: number;
  trip_date: string;
}

const BMF_RATES = [
  { code: 'AT', name: 'Österreich (Inland)', daily: 26.40, hourly: 2.20, lunch: 13.20, dinner: 13.20, breakfast: 3.96 },
  { code: 'DE', name: 'Deutschland', daily: 35.30, hourly: 2.94, lunch: 14.12, dinner: 14.12, breakfast: 5.30 },
  { code: 'CH', name: 'Schweiz', daily: 42.40, hourly: 3.53, lunch: 16.96, dinner: 16.96, breakfast: 6.36 },
  { code: 'IT', name: 'Italien', daily: 36.00, hourly: 3.00, lunch: 14.40, dinner: 14.40, breakfast: 5.40 },
  { code: 'GB', name: 'Großbritannien', daily: 40.00, hourly: 3.33, lunch: 16.00, dinner: 16.00, breakfast: 6.00 },
  { code: 'CZ', name: 'Tschechien', daily: 32.00, hourly: 2.67, lunch: 12.80, dinner: 12.80, breakfast: 4.80 },
  { code: 'HU', name: 'Ungarn', daily: 29.00, hourly: 2.42, lunch: 11.60, dinner: 11.60, breakfast: 4.35 },
  { code: 'US', name: 'USA', daily: 52.00, hourly: 4.33, lunch: 20.80, dinner: 20.80, breakfast: 7.80 },
];

export default function App() {
  const [tab, setTab] = useState<'claims' | 'trips' | 'calc' | 'sepa'>('claims');
  const [selectedClaim, setSelectedClaim] = useState<Claim | null>(null);

  // Mock initial claims state for first-paint telemetry
  const [claims, setClaims] = useState<Claim[]>([
    {
      id: 'EXP-20261101-01',
      employee_id: 'EMP-01',
      employee_name: 'Sandra Schipal',
      category: 'Travel & Subsistence',
      amount_cents: 12000,
      net_cents: 10000,
      tax_cents: 2000,
      tax_key: 'U20',
      is_kleinbetragsrechnung: true,
      status: 'approved',
      claim_date: '2026-11-01',
      notes: 'Customer on-site architecture audit in Linz'
    },
    {
      id: 'EXP-20261104-02',
      employee_id: 'EMP-02',
      employee_name: 'Thomas Craft',
      category: 'Hardware & Accessories',
      amount_cents: 34990,
      net_cents: 29158,
      tax_cents: 5832,
      tax_key: 'U20',
      is_kleinbetragsrechnung: true,
      status: 'submitted',
      claim_date: '2026-11-04',
      notes: 'USB-C Testing Hub & cables'
    }
  ]);

  const [trips] = useState<Trip[]>([
    {
      id: 'TRIP-001',
      origin: 'Wien (HQ)',
      destination: 'Linz (Kunde)',
      distance_km: 184.0,
      vehicle: 'car',
      passengers: 1,
      allowance_cents: 8648,
      trip_date: '2026-11-01'
    }
  ]);

  // Reisekosten calculator state
  const [calcCountry, setCalcCountry] = useState('AT');
  const [calcHours, setCalcHours] = useState(8.5);
  const [calcBreakfast, setCalcBreakfast] = useState(false);
  const [calcLunch, setCalcLunch] = useState(false);
  const [calcDinner, setCalcDinner] = useState(false);
  const [calcKm, setCalcKm] = useState(120);

  const selectedRate = BMF_RATES.find(r => r.code === calcCountry) || BMF_RATES[0];
  const calcPerDiem = (() => {
    if (calcHours < 3.0) return 0;
    const base = calcHours >= 11.0 ? selectedRate.daily : Math.ceil(calcHours) * selectedRate.hourly;
    let deduct = 0;
    if (calcBreakfast) deduct += selectedRate.breakfast;
    if (calcLunch) deduct += selectedRate.lunch;
    if (calcDinner) deduct += selectedRate.dinner;
    return Math.max(0, base - deduct);
  })();
  const calcMileage = calcKm * 0.42;
  const calcTotal = calcPerDiem + calcMileage;

  const totalCents = claims.reduce((acc, c) => acc + c.amount_cents, 0);
  const approvedCents = claims.filter(c => c.status === 'approved').reduce((acc, c) => acc + c.amount_cents, 0);
  const pendingCount = claims.filter(c => c.status === 'submitted').length;

  return (
    <div className="min-h-screen flex flex-col bg-slate-950 text-slate-100">
      {/* Top Header */}
      <header className="border-b border-slate-800 bg-slate-900/80 backdrop-blur sticky top-0 z-50">
        <div className="max-w-7xl mx-auto px-4 py-3 flex items-center justify-between">
          <div className="flex items-center space-x-3">
            <span className="text-2xl">💳</span>
            <div>
              <div className="flex items-center space-x-2">
                <h1 className="font-bold text-lg text-amber-400">SpendCraft</h1>
                <span className="text-xs bg-amber-500/10 text-amber-400 border border-amber-500/20 px-2 py-0.5 rounded-full font-medium">
                  Webapp Companion :11262
                </span>
                <span className="text-xs bg-emerald-500/10 text-emerald-400 border border-emerald-500/20 px-2 py-0.5 rounded-full font-medium">
                  Austrian §26 EStG
                </span>
              </div>
              <p className="text-xs text-slate-400">Sovereign Travel Allowance & SEPA Reimbursement Engine</p>
            </div>
          </div>
          <div className="flex items-center space-x-2 text-xs">
            <span className="px-2.5 py-1 rounded bg-slate-800 text-slate-300 border border-slate-700">
              Backend Port: 11263
            </span>
          </div>
        </div>

        {/* Navigation Tabs */}
        <div className="max-w-7xl mx-auto px-4 flex space-x-1 border-t border-slate-800/60 pt-1">
          <button
            onClick={() => setTab('claims')}
            className={`px-4 py-2.5 text-sm font-medium border-b-2 flex items-center space-x-2 transition ${
              tab === 'claims'
                ? 'border-amber-400 text-amber-400 bg-slate-800/40'
                : 'border-transparent text-slate-400 hover:text-slate-200'
            }`}
          >
            <Receipt className="w-4 h-4" />
            <span>Claims Inbox</span>
          </button>
          <button
            onClick={() => setTab('trips')}
            className={`px-4 py-2.5 text-sm font-medium border-b-2 flex items-center space-x-2 transition ${
              tab === 'trips'
                ? 'border-amber-400 text-amber-400 bg-slate-800/40'
                : 'border-transparent text-slate-400 hover:text-slate-200'
            }`}
          >
            <Car className="w-4 h-4" />
            <span>Fahrtenbuch (Trips)</span>
          </button>
          <button
            onClick={() => setTab('calc')}
            className={`px-4 py-2.5 text-sm font-medium border-b-2 flex items-center space-x-2 transition ${
              tab === 'calc'
                ? 'border-amber-400 text-amber-400 bg-slate-800/40'
                : 'border-transparent text-slate-400 hover:text-slate-200'
            }`}
          >
            <Calculator className="w-4 h-4" />
            <span>§26 EStG Engine</span>
          </button>
          <button
            onClick={() => setTab('sepa')}
            className={`px-4 py-2.5 text-sm font-medium border-b-2 flex items-center space-x-2 transition ${
              tab === 'sepa'
                ? 'border-amber-400 text-amber-400 bg-slate-800/40'
                : 'border-transparent text-slate-400 hover:text-slate-200'
            }`}
          >
            <CreditCard className="w-4 h-4" />
            <span>SEPA Batches</span>
          </button>
        </div>
      </header>

      {/* Main Container */}
      <main className="max-w-7xl mx-auto px-4 py-6 flex-1 w-full space-y-6">
        {/* Executive KPI Header */}
        <div className="grid grid-cols-1 md:grid-cols-4 gap-4">
          <div className="bg-slate-900 border border-slate-800 rounded-lg p-4">
            <span className="text-xs font-medium text-slate-400">Total Claim Volume</span>
            <div className="text-2xl font-bold text-slate-100 mt-1">€{(totalCents / 100).toFixed(2)}</div>
            <span className="text-xs text-slate-500">All registered expenses</span>
          </div>
          <div className="bg-slate-900 border border-slate-800 rounded-lg p-4">
            <span className="text-xs font-medium text-slate-400">Approved Volume</span>
            <div className="text-2xl font-bold text-emerald-400 mt-1">€{(approvedCents / 100).toFixed(2)}</div>
            <span className="text-xs text-emerald-500/80">Ready for SEPA payment</span>
          </div>
          <div className="bg-slate-900 border border-slate-800 rounded-lg p-4">
            <span className="text-xs font-medium text-slate-400">Pending Audit</span>
            <div className="text-2xl font-bold text-amber-400 mt-1">{pendingCount}</div>
            <span className="text-xs text-amber-500/80">Require clerk review</span>
          </div>
          <div className="bg-slate-900 border border-slate-800 rounded-lg p-4">
            <span className="text-xs font-medium text-slate-400">Fahrtenbuch Trips</span>
            <div className="text-2xl font-bold text-blue-400 mt-1">{trips.length}</div>
            <span className="text-xs text-blue-500/80">Logged statutory trips</span>
          </div>
        </div>

        {/* Tab 1: Claims Inbox */}
        {tab === 'claims' && (
          <div className="grid grid-cols-1 lg:grid-cols-3 gap-6">
            <div className="lg:col-span-2 bg-slate-900 border border-slate-800 rounded-lg overflow-hidden">
              <div className="px-4 py-3 border-b border-slate-800 flex justify-between items-center">
                <h2 className="font-semibold text-slate-200">Claims Registry</h2>
                <span className="text-xs text-slate-400">{claims.length} claims</span>
              </div>
              <div className="overflow-x-auto">
                <table className="w-full text-left text-sm">
                  <thead className="bg-slate-800/50 text-slate-400 text-xs uppercase tracking-wider">
                    <tr>
                      <th className="px-4 py-3">ID</th>
                      <th className="px-4 py-3">Employee</th>
                      <th className="px-4 py-3">Category</th>
                      <th className="px-4 py-3">Tax</th>
                      <th className="px-4 py-3">Gross</th>
                      <th className="px-4 py-3">Status</th>
                      <th className="px-4 py-3">Action</th>
                    </tr>
                  </thead>
                  <tbody className="divide-y divide-slate-800">
                    {claims.map(c => (
                      <tr 
                        key={c.id} 
                        onClick={() => setSelectedClaim(c)}
                        className={`hover:bg-slate-800/40 cursor-pointer transition ${
                          selectedClaim?.id === c.id ? 'bg-amber-500/10' : ''
                        }`}
                      >
                        <td className="px-4 py-3 font-mono text-xs text-slate-300">{c.id}</td>
                        <td className="px-4 py-3 font-medium text-slate-200">{c.employee_name}</td>
                        <td className="px-4 py-3 text-slate-400">{c.category}</td>
                        <td className="px-4 py-3 font-mono text-xs text-slate-400">{c.tax_key}</td>
                        <td className="px-4 py-3 font-medium text-emerald-400">€{(c.amount_cents / 100).toFixed(2)}</td>
                        <td className="px-4 py-3">
                          <span className={`px-2 py-0.5 rounded-full text-xs font-medium ${
                            c.status === 'approved' ? 'bg-emerald-500/10 text-emerald-400 border border-emerald-500/20' :
                            c.status === 'submitted' ? 'bg-amber-500/10 text-amber-400 border border-amber-500/20' :
                            'bg-slate-700 text-slate-300'
                          }`}>
                            {c.status}
                          </span>
                        </td>
                        <td className="px-4 py-3">
                          <button className="text-xs text-amber-400 hover:underline">Review</button>
                        </td>
                      </tr>
                    ))}
                  </tbody>
                </table>
              </div>
            </div>

            {/* Claim Inspector */}
            <div className="bg-slate-900 border border-slate-800 rounded-lg p-5">
              <h3 className="font-semibold text-slate-200 mb-4 pb-2 border-b border-slate-800">
                Claim Inspector
              </h3>
              {selectedClaim ? (
                <div className="space-y-4 text-sm">
                  <div>
                    <span className="text-xs text-slate-400">Claim ID</span>
                    <div className="font-mono text-slate-200">{selectedClaim.id}</div>
                  </div>
                  <div>
                    <span className="text-xs text-slate-400">Employee</span>
                    <div className="text-slate-200 font-medium">{selectedClaim.employee_name} ({selectedClaim.employee_id})</div>
                  </div>
                  <div>
                    <span className="text-xs text-slate-400">Category & Date</span>
                    <div className="text-slate-300">{selectedClaim.category} • {selectedClaim.claim_date}</div>
                  </div>
                  <div className="pt-2 border-t border-slate-800">
                    <div className="text-xs text-slate-400">Amounts (Austrian UStG)</div>
                    <div className="text-xl font-bold text-emerald-400 mt-1">€{(selectedClaim.amount_cents / 100).toFixed(2)}</div>
                    <div className="text-xs text-slate-400 mt-1">
                      Net: €{(selectedClaim.net_cents / 100).toFixed(2)} | VAT: €{(selectedClaim.tax_cents / 100).toFixed(2)} ({selectedClaim.tax_key})
                    </div>
                    {selectedClaim.is_kleinbetragsrechnung && (
                      <span className="inline-block mt-2 text-xs bg-blue-500/10 text-blue-400 border border-blue-500/20 px-2 py-0.5 rounded">
                        Kleinbetragsrechnung (&lt; €400)
                      </span>
                    )}
                  </div>
                  {selectedClaim.notes && (
                    <div className="pt-2 border-t border-slate-800">
                      <span className="text-xs text-slate-400">Business Justification</span>
                      <div className="text-xs text-slate-300 mt-1 bg-slate-950 p-2.5 rounded border border-slate-800">
                        {selectedClaim.notes}
                      </div>
                    </div>
                  )}
                  <div className="pt-4 flex space-x-2">
                    <button 
                      onClick={() => {
                        setClaims(claims.map(c => c.id === selectedClaim.id ? { ...c, status: 'approved' } : c));
                        setSelectedClaim({ ...selectedClaim, status: 'approved' });
                      }}
                      className="flex-1 bg-emerald-600 hover:bg-emerald-500 text-white text-xs font-semibold py-2 rounded transition"
                    >
                      ✓ Approve
                    </button>
                    <button 
                      onClick={() => {
                        setClaims(claims.map(c => c.id === selectedClaim.id ? { ...c, status: 'rejected' } : c));
                        setSelectedClaim({ ...selectedClaim, status: 'rejected' });
                      }}
                      className="px-3 bg-rose-600/20 hover:bg-rose-600/30 text-rose-400 border border-rose-600/30 text-xs font-semibold py-2 rounded transition"
                    >
                      Reject
                    </button>
                  </div>
                </div>
              ) : (
                <div className="text-center py-12 text-slate-500 text-xs">
                  Select a claim from the table to view Austrian VAT details and audit status.
                </div>
              )}
            </div>
          </div>
        )}

        {/* Tab 2: Fahrtenbuch */}
        {tab === 'trips' && (
          <div className="grid grid-cols-1 lg:grid-cols-2 gap-6">
            <div className="bg-slate-900 border border-slate-800 rounded-lg p-5">
              <h2 className="font-semibold text-slate-200 mb-1">Fahrtenbuch Trip Registry</h2>
              <p className="text-xs text-slate-400 mb-4">Official Austrian §26 Z4 EStG logbook records</p>
              <div className="space-y-3">
                {trips.map(t => (
                  <div key={t.id} className="bg-slate-950 p-4 rounded-lg border border-slate-800 flex justify-between items-center">
                    <div>
                      <div className="text-xs text-slate-400">{t.trip_date}</div>
                      <div className="font-medium text-slate-200 mt-0.5">{t.origin} → {t.destination}</div>
                      <div className="text-xs text-slate-400 mt-1">
                        {t.distance_km} km • {t.vehicle} • {t.passengers} passenger(s)
                      </div>
                    </div>
                    <div className="text-right">
                      <div className="text-emerald-400 font-bold">€{(t.allowance_cents / 100).toFixed(2)}</div>
                      <span className="text-xs text-slate-500">Kilometergeld</span>
                    </div>
                  </div>
                ))}
              </div>
            </div>

            <div className="bg-slate-900 border border-slate-800 rounded-lg p-5">
              <h2 className="font-semibold text-slate-200 mb-1">Log Business Trip</h2>
              <p className="text-xs text-slate-400 mb-4">Statutory audit-compliant trip entry</p>
              <div className="space-y-3 text-sm">
                <div>
                  <label className="text-xs text-slate-400 block mb-1">Route (Origin → Destination)</label>
                  <div className="grid grid-cols-2 gap-2">
                    <input type="text" defaultValue="Wien (Büro)" className="bg-slate-950 border border-slate-800 rounded px-3 py-1.5 text-xs text-slate-200" />
                    <input type="text" defaultValue="Linz (Kunde)" className="bg-slate-950 border border-slate-800 rounded px-3 py-1.5 text-xs text-slate-200" />
                  </div>
                </div>
                <div>
                  <label className="text-xs text-slate-400 block mb-1">Vehicle Class</label>
                  <select className="w-full bg-slate-950 border border-slate-800 rounded px-3 py-1.5 text-xs text-slate-200">
                    <option value="car">Car (PKW standard: €0.42/km)</option>
                    <option value="car_2026">Car 2026 Reform (€0.50/km)</option>
                    <option value="bicycle">Bicycle / E-Bike (€0.40/km)</option>
                    <option value="motorcycle">Motorcycle (€0.24/km)</option>
                  </select>
                </div>
                <div>
                  <label className="text-xs text-slate-400 block mb-1">Distance (km)</label>
                  <input type="number" defaultValue="184" className="w-full bg-slate-950 border border-slate-800 rounded px-3 py-1.5 text-xs text-slate-200" />
                </div>
                <button 
                  onClick={() => alert("Trip recorded into sovereign Fahrtenbuch")}
                  className="w-full mt-2 bg-amber-500 hover:bg-amber-400 text-slate-950 font-semibold py-2 rounded text-xs transition"
                >
                  Record Trip
                </button>
              </div>
            </div>
          </div>
        )}

        {/* Tab 3: §26 EStG Reisekosten Engine */}
        {tab === 'calc' && (
          <div className="grid grid-cols-1 lg:grid-cols-2 gap-6">
            <div className="bg-slate-900 border border-slate-800 rounded-lg p-5 space-y-4">
              <div>
                <h2 className="font-semibold text-slate-200">Austrian §26 EStG Statutory Engine</h2>
                <p className="text-xs text-slate-400">BMF Auslandsreisesätze with statutory meal deductions</p>
              </div>

              <div>
                <label className="text-xs text-slate-400 block mb-1">Destination Country</label>
                <select 
                  value={calcCountry} 
                  onChange={e => setCalcCountry(e.target.value)}
                  className="w-full bg-slate-950 border border-slate-800 rounded px-3 py-2 text-xs text-slate-200"
                >
                  {BMF_RATES.map(r => (
                    <option key={r.code} value={r.code}>
                      {r.name} — €{r.daily.toFixed(2)}/day (pro-rata €{r.hourly.toFixed(2)}/h)
                    </option>
                  ))}
                </select>
              </div>

              <div>
                <label className="text-xs text-slate-400 block mb-1">
                  Trip Duration: {calcHours} hours
                </label>
                <input 
                  type="range" 
                  min="0" 
                  max="24" 
                  step="0.5" 
                  value={calcHours} 
                  onChange={e => setCalcHours(parseFloat(e.target.value))}
                  className="w-full"
                />
                <span className="text-xs text-slate-500">Threshold: &gt;3 hours pro-rata, &gt;=11 hours full day cap</span>
              </div>

              <div className="pt-2 border-t border-slate-800 space-y-2">
                <span className="text-xs text-slate-400 font-medium block">Provided Meals (Statutory Deductions)</span>
                <label className="flex items-center space-x-2 text-xs text-slate-300">
                  <input type="checkbox" checked={calcBreakfast} onChange={e => setCalcBreakfast(e.target.checked)} />
                  <span>Breakfast provided by hotel (-15%: €{selectedRate.breakfast.toFixed(2)})</span>
                </label>
                <label className="flex items-center space-x-2 text-xs text-slate-300">
                  <input type="checkbox" checked={calcLunch} onChange={e => setCalcLunch(e.target.checked)} />
                  <span>Lunch provided (-50%: €{selectedRate.lunch.toFixed(2)})</span>
                </label>
                <label className="flex items-center space-x-2 text-xs text-slate-300">
                  <input type="checkbox" checked={calcDinner} onChange={e => setCalcDinner(e.target.checked)} />
                  <span>Dinner provided (-50%: €{selectedRate.dinner.toFixed(2)})</span>
                </label>
              </div>

              <div className="pt-2 border-t border-slate-800">
                <label className="text-xs text-slate-400 block mb-1">Kilometergeld Mileage (km)</label>
                <input 
                  type="number" 
                  value={calcKm} 
                  onChange={e => setCalcKm(parseFloat(e.target.value) || 0)}
                  className="w-full bg-slate-950 border border-slate-800 rounded px-3 py-1.5 text-xs text-slate-200"
                />
              </div>
            </div>

            <div className="bg-slate-900 border border-slate-800 rounded-lg p-5 flex flex-col justify-between">
              <div>
                <h3 className="font-semibold text-slate-200 mb-1">Statutory Payout Breakdown</h3>
                <p className="text-xs text-slate-400 mb-6">Tax-free under §26 EStG</p>

                <div className="space-y-4">
                  <div className="flex justify-between items-center py-2 border-b border-slate-800 text-sm">
                    <span className="text-slate-400">Tagesdiäten ({calcCountry}):</span>
                    <span className="font-semibold text-slate-200">€{calcPerDiem.toFixed(2)}</span>
                  </div>
                  <div className="flex justify-between items-center py-2 border-b border-slate-800 text-sm">
                    <span className="text-slate-400">Kilometergeld ({calcKm} km @ €0.42):</span>
                    <span className="font-semibold text-slate-200">€{calcMileage.toFixed(2)}</span>
                  </div>
                  <div className="flex justify-between items-center py-3 text-base">
                    <span className="font-bold text-slate-100">Total Statutory Allowance:</span>
                    <span className="font-bold text-2xl text-emerald-400">€{calcTotal.toFixed(2)}</span>
                  </div>
                </div>
              </div>

              <button 
                onClick={() => {
                  const newClaim: Claim = {
                    id: `EXP-CALC-${Date.now().toString().slice(-6)}`,
                    employee_id: 'EMP-01',
                    employee_name: 'Sandra Schipal',
                    category: 'travel_per_diem',
                    amount_cents: Math.round(calcTotal * 100),
                    net_cents: Math.round(calcTotal * 100),
                    tax_cents: 0,
                    tax_key: 'NONE',
                    is_kleinbetragsrechnung: true,
                    status: 'submitted',
                    claim_date: new Date().toISOString().split('T')[0],
                    notes: `Austrian §26 EStG calculation: ${calcHours}h in ${calcCountry} (€${calcPerDiem.toFixed(2)}), ${calcKm}km mileage (€${calcMileage.toFixed(2)})`
                  };
                  setClaims([newClaim, ...claims]);
                  setTab('claims');
                }}
                className="w-full bg-emerald-600 hover:bg-emerald-500 text-white font-semibold py-2.5 rounded text-xs transition"
              >
                📥 Convert into Reimbursable Claim
              </button>
            </div>
          </div>
        )}

        {/* Tab 4: SEPA Batches */}
        {tab === 'sepa' && (
          <div className="bg-slate-900 border border-slate-800 rounded-lg p-5">
            <h2 className="font-semibold text-slate-200 mb-1">ISO 20022 SEPA Batch Generator</h2>
            <p className="text-xs text-slate-400 mb-6">Generates pain.001.001.03 XML for George (Erste Bank), ELBA, Bank Austria</p>

            <div className="bg-slate-950 p-4 rounded border border-slate-800 font-mono text-xs text-slate-300">
              <div className="text-emerald-400 mb-2">&lt;?xml version="1.0" encoding="UTF-8"?&gt;</div>
              <div className="text-slate-400">&lt;Document xmlns="urn:iso:std:iso:20022:tech:xsd:pain.001.001.03"&gt;</div>
              <div className="pl-4 text-slate-300">&lt;CstmrCdtTrfInitn&gt;</div>
              <div className="pl-8 text-slate-400">&lt;GrpHdr&gt;&lt;NbOfTxs&gt;{claims.filter(c => c.status === 'approved').length}&lt;/NbOfTxs&gt;&lt;CtrlSum&gt;{(approvedCents / 100).toFixed(2)}&lt;/CtrlSum&gt;&lt;/GrpHdr&gt;</div>
              <div className="pl-4 text-slate-300">&lt;/CstmrCdtTrfInitn&gt;</div>
              <div className="text-slate-400">&lt;/Document&gt;</div>
            </div>

            <div className="mt-4 flex space-x-3">
              <button 
                onClick={() => alert("SEPA pain.001.001.03 XML exported for George banking")}
                className="bg-amber-500 hover:bg-amber-400 text-slate-950 font-semibold px-4 py-2 rounded text-xs transition"
              >
                ⚡ Export SEPA Reimbursement Order
              </button>
            </div>
          </div>
        )}
      </main>
    </div>
  );
}
