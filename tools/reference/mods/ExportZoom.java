// CPU-only oracle executed from the user's downloaded Zoomify 2.17.0 JAR.
import dev.isxander.zoomify.zoom.*;
import dev.isxander.zoomify.utils.TransitionType;
import com.google.gson.*;
import java.nio.file.*;
public class ExportZoom {
 public static void main(String[] args)throws Exception{
  var out=new JsonObject();var curves=new JsonArray();
  for(var type:TransitionType.values()) {var row=new JsonObject();row.addProperty("name",type.name().toLowerCase());row.addProperty("inverse",type.hasInverse());row.addProperty("opposite",type.opposite().name().toLowerCase());var samples=new JsonArray();for(int i=0;i<=100;i++){var sample=new JsonArray();double x=i/100.;sample.add(x);sample.add(type.apply(x));if(type.hasInverse())sample.add(type.inverse(x));samples.add(sample);}row.add("samples",samples);curves.add(row);}out.add("curves",curves);
  var helper=new ZoomHelper(new TransitionInterpolator(()->TransitionType.EASE_OUT_EXP,()->TransitionType.EASE_OUT_EXP,()->1.,()->.5),new SmoothInterpolator(()->.7),()->4,()->150,()->10);
  out.addProperty("min_tier",helper.getMinScrollTiers());var trace=new JsonArray();
  for(int i=0;i<100;i++){
   boolean hold=i>=3&&i<45 || i>=60&&i<74;int tier=i<15?0:i<25?3:i<35?-4:i<45?10:i<65?0:i<74?4:0;boolean reset=i==45||i==74;if(reset)helper.reset();helper.tick(hold,tier,.05);var row=new JsonObject();row.addProperty("hold",hold);row.addProperty("tier",tier);row.addProperty("reset",reset);var divisors=new JsonArray();for(float alpha:new float[]{0,.25f,.5f,1})divisors.add(helper.getZoomDivisor(alpha));row.add("divisors",divisors);trace.add(row);
  }out.add("trace",trace);Files.writeString(Path.of(args[0]),new Gson().toJson(out));System.out.println("Exported Zoomify curves and trace");
 }
}
