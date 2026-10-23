// CPU-only oracle from the downloaded 3d-Skin-Layers 1.11.4 JAR.
import dev.tr7zw.skinlayers.versionless.util.wrapper.*;
import dev.tr7zw.skinlayers.versionless.util.wrapper.SolidPixelWrapper.UV;
import dev.tr7zw.skinlayers.versionless.util.Direction;
import dev.tr7zw.skinlayers.versionless.ModBase;
import dev.tr7zw.skinlayers.versionless.config.Config;
import com.google.gson.*;
import java.nio.file.*;
import java.util.*;
public class ExportSkin {
 static int alpha(int u,int v,int pattern){if(u<0||u>=64||v<0||v>=64)return 0;return switch(pattern){case 0->0;case 1->255;case 2->(u+v)%2==0?255:0;case 3->(u*17+v*31)%11<3?255:0;case 4->(u*7+v*13)%3==0?128:255;default->u==12&&v==9?255:0;};}
 static class Capture implements ModelBuilder {
  JsonArray boxes=new JsonArray();int u,v;boolean vanilla;
  public ModelBuilder textureSize(int w,int h){return this;}public ModelBuilder uv(int u,int v){this.u=u;this.v=v;return this;}public ModelBuilder mirror(boolean mirror){return this;}
  public boolean isEmpty(){return boxes.isEmpty();}
  public ModelBuilder addVanillaBox(float x,float y,float z,float w,float h,float d){vanilla=true;return this;}
  public ModelBuilder addBox(float x,float y,float z,float size,Direction[] hidden,Direction[][] corners){var row=new JsonObject();var position=new JsonArray();position.add(x);position.add(y);position.add(z);row.add("position",position);var uv=new JsonArray();uv.add(u);uv.add(v);row.add("uv",uv);int mask=0;for(var dir:hidden)mask|=1<<dir.ordinal();row.addProperty("hidden",mask);var cuts=new JsonArray();for(var pair:corners){var cut=new JsonArray();for(var dir:pair)cut.add(dir.ordinal());cuts.add(cut);}row.add("corners",cuts);boxes.add(row);return this;}
 }
 public static void main(String[] args)throws Exception{
  ModBase.config=new Config();var result=new JsonArray();
  for(int[] dims:new int[][]{{8,8,8},{8,12,4},{4,12,4},{3,12,4}})for(int pattern=0;pattern<6;pattern++)for(boolean fast:new boolean[]{false,true}){
   ModBase.config.fastRender=fast;final int selected=pattern;var texture=new TextureData(){public boolean isPresent(UV uv){return alpha(uv.u(),uv.v(),selected)>0;}public boolean isSolid(UV uv){return alpha(uv.u(),uv.v(),selected)==255;}};
   var capture=new Capture();if(SolidPixelWrapper.wrapBox(capture,texture,dims[0],dims[1],dims[2],8,4,true,0)==null)throw new IllegalStateException("wrap failed");
   var row=new JsonObject();row.add("size",new Gson().toJsonTree(dims));row.addProperty("pattern",pattern);row.addProperty("fast",fast);row.addProperty("vanilla",capture.vanilla);row.add("boxes",capture.boxes);result.add(row);
  }
  Files.writeString(Path.of(args[0]),new Gson().toJson(result));System.out.println("Exported 48 skin voxel cases");
 }
}
