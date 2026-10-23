import {describe,it,expect} from 'vitest';
import {demoSnapshot,radius,snapshotSchema,score,sol,tokenUrl} from './data';
describe('feed contract and sizing',()=>{
 it('validates a standalone demo and ranks both metrics',()=>{const d=snapshotSchema.parse(demoSnapshot(1700000000000));for(const metric of ['net_flow','buy_volume']as const){expect(d[metric]).toHaveLength(36);for(let i=1;i<d[metric].length;i++)expect(score(d[metric][i-1],metric)).toBeGreaterThanOrEqual(score(d[metric][i],metric));}});
 it('keeps amounts lossless until display and rejects malformed payloads',()=>{const d=demoSnapshot();d.net_flow[0].buy_lamports='18446744073709551615';expect(snapshotSchema.parse(d).net_flow[0].buy_lamports).toBe('18446744073709551615');expect(sol('1000000001')).toBe(1.000000001);d.net_flow[0].net_lamports='NaN';expect(snapshotSchema.safeParse(d).success).toBe(false);});
 it('bounds outliers and adapts to mobile',()=>{expect(radius(1e20,1e20,1200,600,50)).toBeLessThanOrEqual(100);expect(radius(1,100,350,470,50)).toBeGreaterThan(5);expect(radius(25,100,1200,600,50)).toBeLessThan(radius(100,100,1200,600,50));});
 it('does not turn malicious metadata into a URL',()=>{const t=demoSnapshot().net_flow[0];expect(tokenUrl({...t,mint:'javascript:alert(1)'})).toBeUndefined();expect(tokenUrl(t)).toMatch(/^https:\/\/pump.fun\/coin\//);});
});
