import test from 'node:test';
import assert from 'node:assert/strict';
import { spawnSync } from 'node:child_process';
import { fileURLToPath } from 'node:url';

test('prepared synthetic payload copy is bounded private exact and non-launching', () => {
  const helper = fileURLToPath(new URL('../scripts/measure-release.py', import.meta.url));
  const result = spawnSync('python3', ['-B', '-c', `
import sys, pathlib, tempfile, os, json, hashlib, importlib.util
spec=importlib.util.spec_from_file_location('observer', sys.argv[1]); m=importlib.util.module_from_spec(spec); spec.loader.exec_module(m)
def forbid(event,args):
    if event in ('subprocess.Popen','os.system','os.posix_spawn','socket.__new__','socket.connect'): raise AssertionError(event)
sys.addaudithook(forbid)
with tempfile.TemporaryDirectory(prefix='synthetic-resource-', dir=os.environ.get('TMPDIR')) as tmp:
    base=pathlib.Path(tmp).resolve(); src=base/'source'; src.mkdir(mode=0o700)
    run='10000000-0000-0000-0000-000000000001'; rel=f'inspection/{run}/journal'
    (src/'inspection').mkdir(mode=0o700); (src/'inspection'/run).mkdir(mode=0o700)
    files={'definitions.sqlite3': b'SYNTHETIC neverexecuted pure copy bytes', rel:b'SYNTHETIC journal copy bytes'}
    for name,data in files.items(): (src/name).write_bytes(data); (src/name).chmod(0o600)
    manifest={'scenario':'SYNTHETIC-neverexecuted-A-B','run':run,'generator_sha256':'0'*64,'files':{n:{'bytes':len(d),'sha256':hashlib.sha256(d).hexdigest()} for n,d in files.items()}}
    meta=src/'synthetic-fixture.json'; meta.write_text(json.dumps(manifest)); meta.chmod(0o600)
    before={n:(src/n).read_bytes() for n in [*files,'synthetic-fixture.json']}
    dst=base/'copy'; receipt=m.copy_prepared_fixture(src,dst)
    for n,d in files.items():
        assert (dst/n).read_bytes()==d
        assert (dst/n).stat().st_mode & 0o777 == 0o600
    assert receipt['scenario']==manifest['scenario']
    assert dst.stat().st_mode & 0o777 == 0o700
    assert before=={n:(src/n).read_bytes() for n in before}
    def refused(source,target):
        try: m.copy_prepared_fixture(source,target)
        except (ValueError,OSError,m.argparse.ArgumentTypeError): pass
        else: raise AssertionError('unsafe copy admitted')
    refused(src,dst)
    alias=base/'alias'; alias.symlink_to(src); refused(alias,base/'alias-copy')
    refused(pathlib.Path('relative'),base/'relative-copy')
    src.chmod(0o755); refused(src,base/'public-copy'); src.chmod(0o700)
    db=src/'definitions.sqlite3'; db.unlink(); db.symlink_to(dst/'definitions.sqlite3'); refused(src,base/'link-copy'); db.unlink(); db.write_bytes(files['definitions.sqlite3']); db.chmod(0o600)
    manifest['files']['../escape']=manifest['files'].pop(rel); meta.write_text(json.dumps(manifest)); refused(src,base/'escape-copy')
    manifest['files'][rel]=manifest['files'].pop('../escape'); meta.write_text(json.dumps(manifest))
    with db.open('wb') as f: f.truncate(2097153)
    refused(src,base/'large-copy')
    assert not any((base/n).exists() for n in ['alias-copy','relative-copy','public-copy','link-copy','escape-copy','large-copy'])
print('pure-copy positive and safety refusals passed; process/network boundaries forbidden')
`, helper], { encoding: 'utf8' });
  assert.equal(result.status, 0, result.stdout + result.stderr);
  assert.match(result.stdout, /pure-copy positive and safety refusals passed/);
});
