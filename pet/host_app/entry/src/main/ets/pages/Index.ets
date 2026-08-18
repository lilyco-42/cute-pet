import native from 'libcute_pet_host.so';
import { display } from '@kit.ArkUI';
import { common } from '@kit.AbilityKit';

// 触摸坐标: onTouch 给 vp(虚拟像素), Rust 画布需要物理 px → 乘 densityPixels
function vp2px(v: number): number {
  return v * display.getDefaultDisplaySync().densityPixels;
}

// XComponent 控制器: surface 生命周期回调 → NAPI → Rust ohos backend
class PetXComponentController extends XComponentController {
  onStatus: (msg: string) => void = () => {};
  currentW: number = 1256; // 当前 surface 尺寸(键盘避让后变化)
  currentH: number = 2526;
  onSurfaceCreated(surfaceId: string): void {
    this.onStatus(`created id=${surfaceId}`);
    try {
      const ret: number = native.petStart(BigInt(surfaceId), 540, 960);
      this.onStatus(`petStart ret=${ret}`);
    } catch (e) {
      this.onStatus(`petStart ERR ${JSON.stringify(e)}`);
    }
  }

  onSurfaceChanged(surfaceId: string, rect: SurfaceRect): void {
    this.onStatus(`changed ${rect.surfaceWidth}x${rect.surfaceHeight}`);
    this.currentW = rect.surfaceWidth;
    this.currentH = rect.surfaceHeight;
    try {
      native.petResize(rect.surfaceWidth, rect.surfaceHeight);
    } catch (e) {
      this.onStatus(`petResize ERR ${JSON.stringify(e)}`);
    }
  }

  onSurfaceDestroyed(surfaceId: string): void {
    this.onStatus('destroyed');
    try {
      native.petStop();
    } catch (e) {
      this.onStatus(`petStop ERR ${JSON.stringify(e)}`);
    }
  }
}

@Entry
@Component
struct Index {
  @State status: string = 'init';
  // 键盘弹出时 XComponent 高度避让(真实缩放 surface, EGL surface 跟随 native window)
  @State kbHeight: number = 0;
  private controller: PetXComponentController = new PetXComponentController();
  private lastTouchY: number = -1;
  private lastTouchX: number = -1;

  aboutToAppear(): void {
    this.controller.onStatus = (msg: string): void => {
      this.status = msg;
      console.info(`CutePetStatus: ${msg}`);
    };
    // 监听软键盘高度变化(单位 vp) → 缩放 XComponent 让输入框可见
    try {
      const ctx = this.getUIContext().getHostContext() as common.UIAbilityContext;
      const win = ctx?.windowStage?.getMainWindowSync();
      if (win) {
        win.on('keyboardHeightChange', (h: number) => {
          // keyboardHeightChange 单位 px → 传 Rust 布局避让(聊天面板上移)
          console.info(`CutePetKb: px=${h}`);
          this.kbHeight = h;
          try {
            native.petKeyboard(h);
          } catch (e) {
            console.info(`CutePetKb: petKeyboard ERR ${JSON.stringify(e)}`);
          }
        });
      }
    } catch (e) {
      console.info(`CutePetKb: listener ERR ${JSON.stringify(e)}`);
    }
  }

  // 点击是否落在 Rust 聊天输入框区域。
  // 键盘弹出时 Rust 布局上移 kbHeight, 输入框跟着上移
  private isInputFieldArea(px: number, py: number, surfW: number, surfH: number): boolean {
    const kbPx = this.kbHeight; // 0=键盘隐藏
    const inputBottom = surfH - kbPx;
    return py > inputBottom - 300 && py < inputBottom + 40 &&
      px > surfW * 0.05 && px < surfW * 0.85;
  }

  build() {
    Stack() {
      XComponent({
        type: XComponentType.SURFACE,
        controller: this.controller
      })
        .width('100%')
        .height('100%')
        .onTouch((event: TouchEvent) => {
          // 触摸 → Rust(立绘点击/按钮), 输入框区域额外弹系统软键盘(中文 IME)
          console.info(`CutePetTouchRaw: type=${event.type} n=${event.touches.length}`);
          const t = event.touches[0];
          if (t) {
            const px = vp2px(t.x);
            const py = vp2px(t.y);
            if (event.type === TouchType.Down) {
              this.lastTouchX = px;
              this.lastTouchY = py;
              try {
                native.petTouch(px, py, t.id, true);
                // 点输入框 → 弹软键盘(native IME attach)
                // 用当前 surface 尺寸(键盘避让后会缩小)
                const surfW = this.controller.currentW;
                const surfH = this.controller.currentH;
                const inField = this.isInputFieldArea(px, py, surfW, surfH);
                console.info(`CutePetIme: surf=${surfW}x${surfH} touch=(${px},${py}) inField=${inField}`);
                if (inField) {
                  const ret: number = native.petAttachIme(this.getUIContext());
                  console.info(`CutePetIme: attach ret=${ret}`);
                }
              } catch (e) {
                console.info(`CutePetTouch: down ERR ${JSON.stringify(e)}`);
              }
            } else if (event.type === TouchType.Up) {
              try {
                native.petTouch(px, py, t.id, false);
              } catch (e) {
                console.info(`CutePetTouch: up ERR ${JSON.stringify(e)}`);
              }
            }
          }
        })
        .onKeyEvent((event: KeyEvent) => {
          // 物理键盘通道(unicode 优先, keyCode 映射 ASCII, 特殊键 petKey)
          if (event.type !== KeyType.Down) {
            return;
          }
          if (event.keyCode === 2055 || event.keyCode === 2054 || event.keyCode === 2049) {
            native.petKey(event.keyCode, true);
            return;
          }
          let ch = 0;
          const uni: number = event.unicode ?? 0;
          if (uni > 0) {
            ch = uni;
          } else if (event.keyCode >= 2000 && event.keyCode <= 2009) {
            ch = 48 + (event.keyCode - 2000);
          } else if (event.keyCode >= 2017 && event.keyCode <= 2042) {
            ch = 65 + (event.keyCode - 2017);
            if (!event.isCapsLockOn) {
              ch += 32;
            }
          }
          if (ch > 0) {
            try {
              native.petChar(ch);
            } catch (e) {
              console.info(`CutePetKey: ERR ${JSON.stringify(e)}`);
            }
          }
        })

      // 顶部状态(调试用)
      Text(this.status)
        .fontSize(14)
        .fontColor(Color.White)
        .margin(4)
        .position({ x: 0, y: 0 })
        .opacity(0.6)
    }
    .width('100%')
    .height('100%')
    .backgroundColor(Color.Black)
  }
}
