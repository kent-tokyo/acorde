# 再生用ミックス設定

段階8の開発用 API。楽譜データとは別の `MixRenderConfig` に音色と処理を保存する。`render_offline_mix_wav_full_sf2` は既存の `OfflineRenderManifest` と SF2 のバイト列を受け取り、PCM16 WAV と `MixRenderReport` を返す。機能を無効にした既定設定では、従来の `render_offline_wav_full_sf2` と同じWAVバイト列を返す。

| 設定 | 音声処理 |
| --- | --- |
| `electric_piano_parts` | 指定パートを呼び出し元のSF2の GM Electric Piano 2、プログラム3へ振り分ける。専用テスト音源のプリセットは2オペレーターFMで作った波形のサンプル再生であり、実時間FM合成器ではない。 |
| `detune` | 周期的に遅延時間を変えたコピーを混ぜる。`cents` は最大25、`wet` は混合率。 |
| `reverb` | 4本の異なる遅延経路と帰還で残響を作る。残響用の末尾時間が足りない指定を拒否する。 |
| `sidechain` | 段階5のキックパートから得た明示的なフレーム時刻で、キック以外の音量を下げる。キックは別バスで合成し、音量制御のあとで戻す。音声検出式コンプレッサーではない。 |

各効果は `None` で独立して無効にする。サイドチェインの時刻が指定パートのキック音高36と一致しない場合、入力エラーを返す。音声の自動化を含む入力は、分離処理で意味を保てないため拒否する。設定・楽譜由来の再生予定・SF2・出力WAVの識別情報をハッシュで返す。出力前のピークからクリッピングの可能性を、末尾10 msの残留音から音の切れを検出する。報告は音楽的な試聴結果ではない。

専用SF2は `scripts/build_rhythm_audition_font.py ... --fm` で生成した `tests/fixtures/arrangement/mix-audition.sf2`。従来音源の生成結果は同じ SHA-256 `303fdd51d36f1c96320bf82d0cea339ddf5794272864af8340ef912837d96342` を維持した。新音源の SHA-256 は `f7884c34012ffa2a000d1a310ede173149e8abc1cda1910c1531b7789d82a792`。

比較用パックは `cargo run -p acorde-cli --example integrated_mix_listening_pack -- <新しい出力ディレクトリ>` で再生成する。固定した入力・設定・8/16小節の各3案の出力ハッシュは [`integrated-stage678-listening-manifest.json`](../tests/fixtures/arrangement/integrated-stage678-listening-manifest.json) に記録した。再生イベントの一致を確認したMusicXML/MSCZと、乾いたWAV、処理後のWAV、ピークを揃えた比較WAVが出力される。生成UUIDが変わるため、書き出した楽譜ファイルのバイト一致は要求しない。

同じパックから `python3 scripts/derive_stage678_native_parity.py <パックのディレクトリ> tests/fixtures/arrangement/stage678-native-parity.json` でネイティブ側の確定設定と発音イベントを固定する。WASMのfull/viewerテストは8/16小節の生成・再現をこの記録と照合する。

局所テストでは、全効果無効時のWAV一致、個別の有効化による発音差、同じ設定でのWAV再生成、無効なキック時刻の拒否、レール値と末尾残留の検出を確認した。8/16小節の各3案を2回生成し、WAVハッシュが一致した。6案のレポートではクリッピングと末尾の音切れは検出されていない。比較用には乾いた音声とミックス音声を線形ピーク0.7に揃えたWAVも出力する。これは知覚上の音量を一致させる処理ではない。試聴とCIは受け入れ前に必要である。
