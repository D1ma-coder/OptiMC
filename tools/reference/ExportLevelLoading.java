// CPU only: original packet codec and LevelLoadTracker state transitions.
import com.google.gson.*;
import java.nio.file.*;
import java.lang.reflect.*;
import java.util.*;
import net.minecraft.client.multiplayer.LevelLoadTracker;
import net.minecraft.network.*;
import net.minecraft.network.protocol.game.ClientboundGameEventPacket;
public class ExportLevelLoading {
 static Object state(LevelLoadTracker tracker)throws Exception {var f=LevelLoadTracker.class.getDeclaredField("clientState");f.setAccessible(true);return f.get(tracker);}
 public static void main(String[] args)throws Exception {
  net.minecraft.SharedConstants.tryDetectVersion();net.minecraft.server.Bootstrap.bootStrap();
  var root=new JsonObject();var b=new FriendlyByteBuf(io.netty.buffer.Unpooled.buffer());
  ClientboundGameEventPacket.STREAM_CODEC.encode(b,new ClientboundGameEventPacket(ClientboundGameEventPacket.LEVEL_CHUNKS_LOAD_START,0));
  byte[] body=new byte[b.readableBytes()];b.readBytes(body);b.release();root.addProperty("body",Base64.getEncoder().encodeToString(body));root.addProperty("id",0x26);
  b=new FriendlyByteBuf(io.netty.buffer.Unpooled.buffer());
  ClientboundGameEventPacket.STREAM_CODEC.encode(b,new ClientboundGameEventPacket(ClientboundGameEventPacket.LEVEL_CHUNKS_LOAD_START,Float.NaN));
  body=new byte[b.readableBytes()];b.readBytes(body);b.release();root.addProperty("ignoredParameterBody",Base64.getEncoder().encodeToString(body));
  var tracker=new LevelLoadTracker();tracker.startClientLoad(null,null);
  root.addProperty("callbackBeforeMarker",tracker.getPlayerCompiledSectionCallback()!=null);
  tracker.loadingPacketsReceived();var chunk=state(tracker);
  root.addProperty("stageAfterMarker",chunk.getClass().getSimpleName());
  var callback=tracker.getPlayerCompiledSectionCallback();root.addProperty("callbackAfterMarker",callback!=null);callback.run();
  tracker.loadingPacketsReceived();root.addProperty("duplicateKeepsState",chunk==state(tracker));
  var ready=chunk.getClass().getDeclaredMethod("playerSectionReady");ready.setAccessible(true);
  root.addProperty("compiled",((java.util.concurrent.atomic.AtomicBoolean)ready.invoke(chunk)).get());
  tracker.startClientLoad(null,null);callback.run();root.addProperty("resetDiscardsOldCallback",state(tracker).getClass().getSimpleName().equals("WaitingForServer"));
  // Force the original timeout short-circuit, so no Minecraft/player/GPU is needed.
  var klass=Class.forName("net.minecraft.client.multiplayer.LevelLoadTracker$WaitingForServer");var ctor=klass.getDeclaredConstructors()[0];ctor.setAccessible(true);
  var field=LevelLoadTracker.class.getDeclaredField("clientState");field.setAccessible(true);field.set(tracker,ctor.newInstance(null,null,net.minecraft.util.Util.getMillis()-1));
  tracker.loadingPacketsReceived();tracker.tickClientLoad();root.addProperty("readyAfterTimeout",tracker.isLevelReady());
  Files.writeString(Path.of(args[0]),new Gson().toJson(root));
 }
}
