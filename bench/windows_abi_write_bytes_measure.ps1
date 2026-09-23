# Set PB_ABI_BENCH_ROOT to an operator-prepared private root containing before/after packages, pblang.exe and the bridge.
& {
$ErrorActionPreference='Stop';$root=$env:PB_ABI_BENCH_ROOT;$rows=@()
if (-not $root -or -not [System.IO.Path]::IsPathRooted($root)) { throw 'explicit absolute benchmark root required' }
foreach ($required in @('pblang.exe','pblang-win64-abi.dll','before\pblang','after\pblang')) {
 if (-not (Test-Path -LiteralPath (Join-Path $root $required))) { throw 'incomplete benchmark package' }
}
foreach($job in @(@{name='small';size=64;rounds=256},@{name='buffer';size=4096;rounds=32})){
 for($i=0;$i -lt 8;$i++){
  $order=if($i%2 -eq 0){@('before','after')}else{@('after','before')}
  foreach($v in $order){
   $si=New-Object System.Diagnostics.ProcessStartInfo;$si.FileName=$root+'\pblang.exe';$si.WorkingDirectory=$root+'\'+$v+'\pblang'
   $si.Arguments='run bench/windows_abi_write_bytes.sapp --allow-all -- '+$job.size+' '+$job.rounds
   $si.UseShellExecute=$false;$si.RedirectStandardOutput=$true;$si.RedirectStandardError=$true
   $si.EnvironmentVariables['PBLANG_WINDOWS_ABI_BRIDGE']=$root+'\pblang-win64-abi.dll';$si.EnvironmentVariables['TMPDIR']=$root
   $p=New-Object System.Diagnostics.Process;$p.StartInfo=$si;$sw=[System.Diagnostics.Stopwatch]::StartNew()
   $null=$p.Start();$o=$p.StandardOutput.ReadToEndAsync();$e=$p.StandardError.ReadToEndAsync()
   if(-not $p.WaitForExit(90000)){$p.Kill();$p.WaitForExit();throw 'benchmark timeout'}
   $sw.Stop();$out=$o.Result;$err=$e.Result
   if($p.ExitCode -ne 0){throw ('benchmark failed '+$v+' '+$err)}
   $rows+=@{variant=$v;workload=$job.name;iteration=$i;warmup=($i -eq 0);cpu_ms=$p.TotalProcessorTime.TotalMilliseconds;elapsed_ms=$sw.Elapsed.TotalMilliseconds;peak_working_set_bytes=$p.PeakWorkingSet64;stdout=$out;stderr=$err}
   $p.Dispose();$rows|ConvertTo-Json -Depth 5|Set-Content -Encoding UTF8 -LiteralPath ($root+'\measurements.json')
  }
 }
}
$rows|ConvertTo-Json -Depth 5
}
